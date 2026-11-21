//! PA07-IMP-004：Run 启动门禁快照与 Product 执行门禁（PA-07 V1.0 §5）。
//!
//! 覆盖：claim 成功落依赖逐边快照（前置 Revision/条件/交付物引用）与 Product
//! 门禁快照；suspended/archived 关联 Product 阻塞领取（release_reason 记录）；
//! Product 暂停与领取的并发竞争由行锁串行化（不存在交错通过）。

use std::sync::Arc;

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_dispatch::{claim_once, ClaimKind, DispatchMode, RunDispatchConfig};
use pm_application::PolicyEngine;
use pm_domain::SYSTEM_TENANT;
use sqlx::{PgPool, Row};
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

/// 可领取任务 + queued Run + 初始 Revision（claim 的完整前置）。
async fn seed_claimable(pool: &PgPool, project_id: Uuid, code: &str) -> Uuid {
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
    .bind(project_id)
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
    .bind(project_id)
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
    .bind(project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
    task_id
}

/// 直插一条 completed 前置边（前置带当前 Revision 快照，供断言引用）。
async fn seed_completed_predecessor(
    pool: &PgPool,
    project_id: Uuid,
    downstream: Uuid,
    code: &str,
    created_by: Uuid,
) -> Uuid {
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', $1) RETURNING id",
    )
    .bind(format!("up-{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let upstream: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, 'up', 'completed', 1, $2, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(agent)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_revision (tenant_id, project_id, task_id, revision_no) \
         SELECT tenant_id, project_id, $1, 1 FROM delivery.task WHERE id = $1 RETURNING id",
    )
    .bind(upstream)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET current_task_revision_id = $2 WHERE id = $1")
        .bind(upstream)
        .bind(revision_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by, required_condition) \
         SELECT tenant_id, project_id, $1, $2, $3, 'completed' \
           FROM delivery.task WHERE id = $1",
    )
    .bind(downstream)
    .bind(upstream)
    .bind(created_by)
    .execute(pool)
    .await
    .unwrap();
    upstream
}

/// 直建 Product（归属该任务租户的新 Team）+ 有效 Task-Product 归属。
async fn seed_task_product(pool: &PgPool, project_id: Uuid, task_id: Uuid, status: &str) -> Uuid {
    let product_id: Uuid = sqlx::query_scalar(
        "WITH t AS ( \
             INSERT INTO core.team (tenant_id, name) \
             SELECT tenant_id, $1 || '-team' FROM delivery.task WHERE id = $2 RETURNING tenant_id, id \
         ), p AS ( \
             INSERT INTO core.product (team_id, tenant_id, key, name, status) \
             SELECT id, tenant_id, 'P-' || upper(substring(gen_random_uuid()::text from 1 for 8)), $1, $3 FROM t RETURNING id \
         ) SELECT id FROM p",
    )
    .bind(format!("p-{status}"))
    .bind(task_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_product (tenant_id, project_id, task_id, product_id) \
         SELECT tenant_id, project_id, $1, $2 FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .bind(product_id)
    .execute(pool)
    .await
    .unwrap();
    product_id
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

async fn run_status(pool: &PgPool, run_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task_run WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn release_reason(pool: &PgPool, run_id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT release_reason FROM delivery.task_run WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// claim 成功 → 依赖逐边快照（前置 Revision/条件/satisfied）与 Product 快照
/// （无关联 = 空数组）同时落库。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_writes_gate_snapshots(pool: PgPool) {
    let f = fx(&pool, "SG-C1").await;
    let downstream = seed_claimable(&pool, f.project_id, "C1").await;
    let upstream = seed_completed_predecessor(&pool, f.project_id, downstream, "C1", f.actor_id).await;
    // 找到该任务的 queued run。
    let run_id: Uuid =
        sqlx::query_scalar("SELECT id FROM delivery.task_run WHERE task_id = $1")
            .bind(downstream)
            .fetch_one(&pool)
            .await
            .unwrap();

    let outcome = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(outcome.run_id, run_id);
    assert_eq!(run_status(&pool, run_id).await, "claimed");

    // 依赖逐边快照。
    let snap = sqlx::query(
        "SELECT predecessor_task_id, predecessor_revision_id, required_condition, \
                required_deliverable_ref, satisfied \
           FROM delivery.task_run_dependency_snapshot WHERE run_id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(snap.get::<Uuid, _>("predecessor_task_id"), upstream);
    let expected_revision: Option<Uuid> = sqlx::query_scalar(
        "SELECT current_task_revision_id FROM delivery.task WHERE id = $1",
    )
    .bind(upstream)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        snap.get::<Option<Uuid>, _>("predecessor_revision_id"),
        expected_revision
    );
    assert_eq!(snap.get::<String, _>("required_condition"), "completed");
    assert_eq!(snap.get::<Option<serde_json::Value>, _>("required_deliverable_ref"), None);
    assert!(snap.get::<bool, _>("satisfied"));

    // Product 快照：无关联 = 空数组。
    let empty: serde_json::Value = sqlx::query_scalar(
        "SELECT product_gate_snapshot FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(empty, serde_json::json!([]));
}

/// suspended/archived 关联 Product 阻塞领取：退避回队 + release_reason，
/// 快照与租约均不落库（失败路径不写证据）。
#[sqlx::test(migrations = "../../migrations")]
async fn blocked_product_denies_claim(pool: PgPool) {
    for status in ["suspended", "archived"] {
        let f = fx(&pool, &format!("SG-B-{status}")).await;
        let task_id = seed_claimable(&pool, f.project_id, "B").await;
        seed_task_product(&pool, f.project_id, task_id, status).await;
        let run_id: Uuid =
            sqlx::query_scalar("SELECT id FROM delivery.task_run WHERE task_id = $1")
                .bind(task_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        let outcome = claim_once(&pool, "w1", &cfg(), DispatchMode::On)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(outcome.kind, ClaimKind::Skipped));
        assert_eq!(run_status(&pool, run_id).await, "queued");
        assert_eq!(
            release_reason(&pool, run_id).await.as_deref(),
            Some(if status == "suspended" { "product_suspended" } else { "product_archived" })
        );
        let snap_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.task_run_dependency_snapshot WHERE run_id = $1",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(snap_count, 0, "denied claim must not write gate snapshots");
    }
}

/// Product 暂停与领取并发：行锁串行化——要么 claim 先提交（快照记录放行且
/// 暂停在其后生效），要么暂停先提交（claim 被拒）。不存在「claim 成功但
/// 快照记录放行而暂停已先行提交」的交错。
#[sqlx::test(migrations = "../../migrations")]
async fn product_suspend_and_claim_race_is_serialized(pool: PgPool) {
    let f = fx(&pool, "SG-R1").await;
    let task_id = seed_claimable(&pool, f.project_id, "R1").await;
    let product_id = seed_task_product(&pool, f.project_id, task_id, "normal").await;
    let run_id: Uuid = sqlx::query_scalar("SELECT id FROM delivery.task_run WHERE task_id = $1")
        .bind(task_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let claim_pool = pool.clone();
    let claim = tokio::spawn(async move {
        claim_once(&claim_pool, "w1", &cfg(), DispatchMode::On)
            .await
            .unwrap()
    });
    let suspend_pool = pool.clone();
    let suspend = tokio::spawn(async move {
        // 直接以 lifecycle 语义执行状态变更（含 FOR UPDATE + 审计 + commit）。
        sqlx::query("UPDATE core.product SET status = 'suspended', updated_at = now() WHERE id = $1")
            .bind(product_id)
            .execute(&suspend_pool)
            .await
            .unwrap();
    });
    let claim_outcome = claim.await.unwrap();
    suspend.await.unwrap();

    match claim_outcome {
        Some(outcome) if matches!(outcome.kind, ClaimKind::Claimed | ClaimKind::Reclaimed) => {
            // claim 先行：快照必须记录放行结论，且暂停发生在快照之后。
            let snapshot: serde_json::Value = sqlx::query_scalar(
                "SELECT product_gate_snapshot FROM delivery.task_run WHERE id = $1",
            )
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
            let entry = &snapshot[0];
            assert_eq!(entry["allowed"], serde_json::json!(true));
            assert_eq!(entry["execution_state"], serde_json::json!("normal"));
            let snapshot_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
                "SELECT product_gate_snapshot_at FROM delivery.task_run WHERE id = $1",
            )
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
            let suspended_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
                "SELECT updated_at FROM core.product WHERE id = $1",
            )
            .bind(product_id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(
                suspended_at >= snapshot_at,
                "suspend must not have committed before the claim snapshot"
            );
        }
        Some(_) => {
            // claim 被拒（Skipped）：暂停先行，run 保持 queued 且 release_reason 命中。
            assert_eq!(run_status(&pool, run_id).await, "queued");
            assert_eq!(
                release_reason(&pool, run_id).await.as_deref(),
                Some("product_suspended")
            );
        }
        None => panic!("a queued runnable run must be observed by claim_once"),
    }
    // 无论交错顺序：product 终态 = suspended。
    let final_status: String =
        sqlx::query_scalar("SELECT status FROM core.product WHERE id = $1")
            .bind(product_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(final_status, "suspended");
}
