//! PA07-IMP-006：Human Consultation 正式决策工作项（PA-07 V1.0 §10.2）。
//!
//! 覆盖：四态状态机合法迁移与非法跳转、decided 四元组 DB CHECK 强制完整、
//! cancel 必填原因、既有决定被改变走 supersede 追加不覆盖、挂任务的咨询经
//! task_waiting('needs_decision') 阻塞领取且 decide 后解除、**未回应不得视为
//! 同意**（无任何自动 decided 路径——pending 长存且 claim 保持拒绝）。

use std::sync::Arc;

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::human_consultation::{
    cancel_consultation, create_consultation, decide_consultation, list_consultations,
    start_consultation, supersede_consultation, ConsultationError,
};
use pm_application::run_dispatch::{claim_once, ClaimKind, DispatchMode, RunDispatchConfig};
use pm_application::PolicyEngine;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    actor_id: Uuid,
}

async fn fx(pool: &PgPool, code: &str) -> Fx {
    let o = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        project_id: o.project_id,
        actor_id: o.human_id,
    }
}

fn actor(fx: &Fx) -> AuditActor {
    AuditActor {
        principal_id: fx.actor_id,
        identity_id: None,
        client: None,
        auth_mode: None,
        claimed_device_id: None,
    }
}

async fn consultation_status(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM governance.human_consultation WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
        timeout_secs: 1800,
        deadline_default_secs: 3600,
        agent_heartbeat_stale_secs: 900,
        progress_stale_secs: 3600,
        exclude_projects: vec![],
    }
}

/// 可领取任务 + queued Run + Revision（与 run_start_gates 同款前置）。
async fn seed_claimable(pool: &PgPool, f: &Fx, code: &str) -> Uuid {
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', $1) RETURNING id",
    )
    .bind(format!("dev {code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, 't', 'ready', 1, $2, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(f.project_id)
    .bind(agent)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_revision (tenant_id, project_id, task_id, revision_no) \
         SELECT tenant_id, project_id, $1, 1 FROM delivery.task WHERE id = $1 RETURNING id",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET current_task_revision_id = $2 WHERE id = $1")
        .bind(task_id)
        .bind(revision_id)
        .execute(pool)
        .await
        .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, $2, $3, 'verified', 'headsha', (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(f.project_id)
    .bind(format!("repo-{code}"))
    .bind(format!("git@host:{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository \
         (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(f.project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(f.project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    task_id
}

async fn active_waiting_reason(pool: &PgPool, task_id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT reason FROM delivery.task_waiting WHERE task_id = $1 AND ended_at IS NULL",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .unwrap()
    .flatten()
}

/// pending → in_progress → decided（四元组完整落库）；decide 后等待解除。
#[sqlx::test(migrations = "../../migrations")]
async fn lifecycle_pending_in_progress_decided(pool: PgPool) {
    let f = fx(&pool, "HC-L1").await;
    let task = seed_claimable(&pool, &f, "L1").await;
    let id = create_consultation(
        &pool,
        SYSTEM_TENANT,
        f.project_id,
        Some(task),
        "是否接受新验收口径",
        "客户要求追加验收项，是否接受？",
        serde_json::json!([]),
        None,
        f.actor_id,
        &actor(&f),
    )
    .await
    .unwrap();
    assert_eq!(consultation_status(&pool, id).await, "pending");
    // 挂任务 → needs_decision 等待行存在（调度阻塞执法点）。
    assert_eq!(
        active_waiting_reason(&pool, task).await.as_deref(),
        Some("needs_decision")
    );

    start_consultation(&pool, SYSTEM_TENANT, id, &actor(&f))
        .await
        .unwrap();
    assert_eq!(consultation_status(&pool, id).await, "in_progress");

    decide_consultation(
        &pool,
        SYSTEM_TENANT,
        id,
        "接受追加验收项，纳入正式基线",
        f.actor_id,
        &actor(&f),
    )
    .await
    .unwrap();
    assert_eq!(consultation_status(&pool, id).await, "decided");
    // decided 四元组完整。
    let (content, by, at, scope): (
        String,
        Uuid,
        chrono::DateTime<chrono::Utc>,
        serde_json::Value,
    ) = sqlx::query_as(
        "SELECT decision_content, decided_by, decided_at, product_scope \
               FROM governance.human_consultation WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(content, "接受追加验收项，纳入正式基线");
    assert_eq!(by, f.actor_id);
    assert_eq!(scope, serde_json::json!([]));
    let _ = at;
    // 决策完成 → 等待解除。
    assert_eq!(active_waiting_reason(&pool, task).await, None);
}

/// decided 缺内容 → 拒（应用层 + DB CHECK 双保险）；cancel 缺原因 → 拒。
#[sqlx::test(migrations = "../../migrations")]
async fn decide_requires_content_and_cancel_requires_reason(pool: PgPool) {
    let f = fx(&pool, "HC-G1").await;
    let id = create_consultation(
        &pool,
        SYSTEM_TENANT,
        f.project_id,
        None,
        "重大资源取舍",
        "是否投入额外设备？",
        serde_json::json!([]),
        None,
        f.actor_id,
        &actor(&f),
    )
    .await
    .unwrap();

    let err = decide_consultation(&pool, SYSTEM_TENANT, id, "   ", f.actor_id, &actor(&f))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ConsultationError::IncompleteDecision),
        "got {err:?}"
    );

    let err = cancel_consultation(&pool, SYSTEM_TENANT, id, "", &actor(&f))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ConsultationError::MissingReason),
        "got {err:?}"
    );

    // 合法 cancel 后再 decide → 非法迁移。
    cancel_consultation(&pool, SYSTEM_TENANT, id, "事项已由线下协商解决", &actor(&f))
        .await
        .unwrap();
    let err = decide_consultation(
        &pool,
        SYSTEM_TENANT,
        id,
        "late decision",
        f.actor_id,
        &actor(&f),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ConsultationError::InvalidTransition(_)),
        "got {err:?}"
    );
}

/// 未回应不得视为同意：pending 长存、无自动 decided 路径；已 decided 的
/// 决定被改变 → supersede 追加新行（原行保留 decided），旧等待解除新等待写入。
#[sqlx::test(migrations = "../../migrations")]
async fn no_response_is_never_consent_and_supersede_appends(pool: PgPool) {
    let f = fx(&pool, "HC-S1").await;
    let task = seed_claimable(&pool, &f, "S1").await;
    let id = create_consultation(
        &pool,
        SYSTEM_TENANT,
        f.project_id,
        Some(task),
        "方向调整决策",
        "A 方案 or B 方案？",
        serde_json::json!([]),
        None,
        f.actor_id,
        &actor(&f),
    )
    .await
    .unwrap();
    // 无任何动作 → 状态停留 pending（结构上不存在自动同意）。
    assert_eq!(consultation_status(&pool, id).await, "pending");
    assert_eq!(
        active_waiting_reason(&pool, task).await.as_deref(),
        Some("needs_decision")
    );
    // 该任务的 Run 不能被领取（claim 门禁 task_waiting 拒绝）。
    let claim = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap();
    assert!(
        matches!(claim, Some(ref o) if o.kind == ClaimKind::Skipped),
        "decision-pending task must not be claimable"
    );

    // 决定后改变主意 → supersede：新 pending 行 + 原行 decided 保留。
    decide_consultation(
        &pool,
        SYSTEM_TENANT,
        id,
        "选 A 方案",
        f.actor_id,
        &actor(&f),
    )
    .await
    .unwrap();
    let new_id = supersede_consultation(
        &pool,
        SYSTEM_TENANT,
        id,
        "方向调整决策 v2",
        "A 方案实施中发现风险，是否改选 B？",
        f.actor_id,
        &actor(&f),
    )
    .await
    .unwrap();
    assert_eq!(consultation_status(&pool, id).await, "decided");
    assert_eq!(consultation_status(&pool, new_id).await, "pending");
    let sup: Option<Uuid> = sqlx::query_scalar(
        "SELECT supersedes_consultation_id FROM governance.human_consultation WHERE id = $1",
    )
    .bind(new_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sup, Some(id));
    // supersede 写入新等待行 → 任务重新阻塞。
    assert_eq!(
        active_waiting_reason(&pool, task).await.as_deref(),
        Some("needs_decision")
    );

    let rows = list_consultations(&pool, SYSTEM_TENANT, f.project_id, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
}
