//! G1 复测（DEC-SEC-P0-01 阶段 A3）：SEC-P0-1～5 跨租户攻击回归 + 合法路径回归。
//!
//! 修复模式：五个 legacy 端点统一 `require_legacy_project_access`
//! （`require_project_visible`，off 模式豁免）+ actor 只来自 `AuthedPrincipal`。
//!
//! 断言口径（交付物 05 V1.1 §2.1/§3.3）：
//! - 写操作：HTTP 拒绝 **且** DB 前后无变化 **且** 无命令执行/补偿副作用；
//! - 读操作（P0-5）：HTTP 拒绝 **且** 响应不含目标租户数据。
//! - 双向：攻击路径拒绝，userA 对自己项目（projectA1）的合法路径仍成功。
//!
//! 阶段 B 起双租户夹具统一到 `l2_security`（租户 A = SYSTEM_TENANT 经 lmms:1
//! identity 映射；租户 B = 独立 tenant 经 bootstrap），本文件只保留 P0 专项种子。

use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod l2_security;

use l2_security::*;

/// 监督链种子：task + audit(补偿快照) + event + run + proposal；executed 追加
/// executed_at/audit_id，proposed 追加 pending approval。tenant 经 project 反查。
async fn seed_proposal_chain(
    pool: &PgPool,
    project_id: Uuid,
    proposal_status: &str,
) -> (Uuid, Option<Uuid>) {
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, title, status, revision, tenant_id) \
         VALUES ($1, $2, 'G1-TASK', 'completed', 4, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();

    // FK：audit_log.actor_id → core.principal（B 侧无 bootstrap，需先播种主体）。
    let actor_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'g1-seed')",
    )
    .bind(actor_id)
    .execute(pool)
    .await
    .unwrap();

    let audit_id = Uuid::new_v4();
    let compensation = serde_json::json!({
        "Restore": [
            { "table": "task", "target_id": task_id,
              "fields": { "status": "running", "revision": 4 } }
        ]
    });
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, action, target_type, target_id, before_data, after_data, compensation_command, tenant_id) \
         VALUES ($1, $2, $3, 'update_task_status', 'task', $4, $5, $6, $7, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(audit_id)
    .bind(project_id)
    .bind(actor_id)
    .bind(task_id)
    .bind(json!({ "status": "running", "revision": 3 }))
    .bind(json!({ "status": "completed", "revision": 4 }))
    .bind(compensation)
    .execute(pool)
    .await
    .unwrap();

    let event_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, tenant_id) \
         VALUES ($1, $2, $3, 'test', 'bug_report', $4, '{}', 'hash', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(event_id)
    .bind(project_id)
    .bind(actor_id)
    .bind(Uuid::new_v4().to_string())
    .execute(pool)
    .await
    .unwrap();
    let run_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai.analysis_run \
         (id, project_id, event_id, model_name, policy_version, config_snapshot, tenant_id) \
         VALUES ($1, $2, $3, 'fake-model', 1, '{}', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(event_id)
    .execute(pool)
    .await
    .unwrap();

    let proposal_id = Uuid::new_v4();
    let proposed_data = json!({
        "kind": "update_task_status",
        "data": { "task_id": task_id, "expected_revision": 4, "new_status": "submitted" }
    });
    if proposal_status == "executed" {
        sqlx::query(
            "INSERT INTO ai.change_proposal \
             (id, project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, \
              proposed_data, rationale, risk_level, status, executed_at, executed_audit_id, tenant_id) \
             VALUES ($1, $2, $3, 0, 'task', $4, 'update_task_status', $5, 'because', 'low', \
                     'executed', now(), $6, \
                     (SELECT tenant_id FROM core.project WHERE id = $2))",
        )
        .bind(proposal_id)
        .bind(project_id)
        .bind(run_id)
        .bind(task_id)
        .bind(&proposed_data)
        .bind(audit_id)
        .execute(pool)
        .await
        .unwrap();
        return (proposal_id, None);
    }
    sqlx::query(
        "INSERT INTO ai.change_proposal \
         (id, project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, \
          proposed_data, rationale, risk_level, status, tenant_id) \
         VALUES ($1, $2, $3, 0, 'task', $4, 'update_task_status', $5, 'because', 'low', 'proposed', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(proposal_id)
    .bind(project_id)
    .bind(run_id)
    .bind(task_id)
    .bind(&proposed_data)
    .execute(pool)
    .await
    .unwrap();
    let approval_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO governance.approval_request \
         (id, project_id, proposal_id, status, required_role, tenant_id) \
         VALUES ($1, $2, $3, 'pending', 'project_owner', \
                 (SELECT tenant_id FROM ai.change_proposal WHERE id = $3))",
    )
    .bind(approval_id)
    .bind(project_id)
    .bind(proposal_id)
    .execute(pool)
    .await
    .unwrap();
    (proposal_id, Some(approval_id))
}

async fn seed_notification(pool: &PgPool, project_id: Uuid, payload: Value) {
    sqlx::query(
        "INSERT INTO integration.notification (project_id, kind, payload, tenant_id) \
         VALUES ($1, 'veto_applied', $2, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(payload)
    .execute(pool)
    .await
    .unwrap();
}

// ------------------------------------------------------------------ SEC-P0-1

/// 攻击：tenantA 用户向 tenantB 项目注入事件 → 404，且无 event/job 落库；
/// 合法路径：userA 对 projectA1 仍 202。
#[sqlx::test(migrations = "../../migrations")]
async fn p0_1_events_cross_tenant_rejected(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            "/api/v1/events",
            Some(&a.token),
            Some(json!({
                "project_id": b.project_id,
                "actor_id": Uuid::new_v4(),
                "source": "attacker",
                "event_type": "bug_report",
                "idempotency_key": "atk-1",
                "payload": {"text": "injected"}
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = status_body(response).await.1;
    assert_eq!(body["error"], json!("project_not_found"));

    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM integration.inbound_event), \
                (SELECT count(*) FROM integration.job)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, 0), "attack must not write event or job rows");

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            "/api/v1/events",
            Some(&a.token),
            Some(json!({
                "project_id": a.project_id,
                "actor_id": Uuid::new_v4(),
                "source": "feishu",
                "event_type": "bug_report",
                "idempotency_key": "legal-1",
                "payload": {"text": "login returns 500"}
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

// ------------------------------------------------------------------ SEC-P0-2

/// 攻击：tenantA 用户裁决 tenantB 项目的 pending approval → 404（不泄露存在性），
/// approval 仍 pending、proposal 仍 proposed、无通知落库；
/// 合法路径：userA 对自己项目的 approval reject → 200，decided_by=认证主体（A-02）。
#[sqlx::test(migrations = "../../migrations")]
async fn p0_2_decision_cross_tenant_rejected(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let (proposal_b, approval_b) = seed_proposal_chain(&pool, b.project_id, "proposed").await;
    let approval_b = approval_b.expect("proposed seed yields a pending approval");

    let uri = format!("/api/v1/supervision/approvals/{approval_b}/decision");
    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &uri,
            Some(&a.token),
            Some(json!({ "approval_id": approval_b, "actor_id": Uuid::new_v4(), "decision": "reject", "reason": "cross tenant" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = status_body(response).await.1;
    assert_eq!(body["error"], json!("approval_not_found"));

    let (approval_status, decided_by): (String, Option<Uuid>) =
        sqlx::query_as("SELECT status, decided_by FROM governance.approval_request WHERE id = $1")
            .bind(approval_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(approval_status, "pending", "approval must stay pending");
    assert_eq!(decided_by, None);
    let proposal_status: String =
        sqlx::query_scalar("SELECT status FROM ai.change_proposal WHERE id = $1")
            .bind(proposal_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proposal_status, "proposed", "proposal must stay untouched");
    let notifications: i64 =
        sqlx::query_scalar("SELECT count(*) FROM integration.notification WHERE project_id = $1")
            .bind(b.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(notifications, 0);

    // 合法路径（同用户、自己项目）。
    let (proposal_a, approval_a) = seed_proposal_chain(&pool, a.project_id, "proposed").await;
    let approval_a = approval_a.expect("proposed seed yields a pending approval");
    let uri = format!("/api/v1/supervision/approvals/{approval_a}/decision");
    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &uri,
            Some(&a.token),
            Some(json!({ "approval_id": approval_a, "actor_id": Uuid::new_v4(), "decision": "reject", "reason": "own project" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = status_body(response).await.1;
    assert_eq!(body["outcome"], json!("rejected"));
    let (approval_status, decided_by): (String, Option<Uuid>) =
        sqlx::query_as("SELECT status, decided_by FROM governance.approval_request WHERE id = $1")
            .bind(approval_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(approval_status, "rejected");
    assert_eq!(
        decided_by,
        Some(a.human_id),
        "decided_by must be the authenticated principal (A-02)"
    );
    let proposal_status: String =
        sqlx::query_scalar("SELECT status FROM ai.change_proposal WHERE id = $1")
            .bind(proposal_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proposal_status, "vetoed");
}

// ------------------------------------------------------------------ SEC-P0-3

/// 攻击：tenantA 用户 veto tenantB 已执行提案 → 404，补偿不发生；
/// 合法路径：userA veto 自己项目的 executed 提案 → 200 rolled_back，补偿真实发生。
#[sqlx::test(migrations = "../../migrations")]
async fn p0_3_veto_cross_tenant_rejected(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let (proposal_b, _) = seed_proposal_chain(&pool, b.project_id, "executed").await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            "/api/v1/supervision/veto",
            Some(&a.token),
            Some(json!({
                "project_id": b.project_id,
                "proposal_id": proposal_b,
                "actor_id": Uuid::new_v4(),
                "reason": "cross tenant"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = status_body(response).await.1;
    assert_eq!(body["error"], json!("project_not_found"));

    let task: (String, i64) = sqlx::query_as(
        "SELECT status, revision FROM delivery.task WHERE project_id = $1 AND title = 'G1-TASK'",
    )
    .bind(b.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        task,
        ("completed".to_string(), 4),
        "no compensation may run"
    );
    let interventions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.human_intervention WHERE project_id = $1",
    )
    .bind(b.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(interventions, 0);
    let notifications: i64 =
        sqlx::query_scalar("SELECT count(*) FROM integration.notification WHERE project_id = $1")
            .bind(b.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(notifications, 0);

    // 合法路径（同用户、自己项目）。
    let (proposal_a, _) = seed_proposal_chain(&pool, a.project_id, "executed").await;
    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            "/api/v1/supervision/veto",
            Some(&a.token),
            Some(json!({
                "project_id": a.project_id,
                "proposal_id": proposal_a,
                "actor_id": Uuid::new_v4(),
                "reason": "own project rollback"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = status_body(response).await.1;
    assert_eq!(body["outcome"], json!("rolled_back"));
    let interventions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.human_intervention \
         WHERE project_id = $1 AND intervention_type = 'veto'",
    )
    .bind(a.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(interventions, 1);
}

// ------------------------------------------------------------------ SEC-P0-4

/// 攻击：tenantA 用户翻转 tenantB 项目的 automation_mode → 404，配置不变；
/// 合法路径：userA 冻结自己的项目 → 200 frozen。
#[sqlx::test(migrations = "../../migrations")]
async fn p0_4_freeze_cross_tenant_rejected(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    // bootstrap 已为两侧项目种下 ai.project_ai_config（automation_mode=ai_lead）。

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            "/api/v1/supervision/freeze",
            Some(&a.token),
            Some(json!({
                "project_id": b.project_id,
                "actor_id": Uuid::new_v4(),
                "reason": "cross tenant"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let mode: String = sqlx::query_scalar(
        "SELECT automation_mode FROM ai.project_ai_config WHERE project_id = $1",
    )
    .bind(b.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mode, "ai_lead", "automation_mode must stay untouched");

    // 合法路径（同用户、自己项目）。
    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            "/api/v1/supervision/freeze",
            Some(&a.token),
            Some(json!({
                "project_id": a.project_id,
                "actor_id": Uuid::new_v4(),
                "reason": "own project brake"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = status_body(response).await.1;
    assert_eq!(body["outcome"], json!("frozen"));
    let mode: String = sqlx::query_scalar(
        "SELECT automation_mode FROM ai.project_ai_config WHERE project_id = $1",
    )
    .bind(a.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mode, "controlled");
}

// ------------------------------------------------------------------ SEC-P0-5

/// 攻击：tenantA 用户读取 tenantB 项目通知 → 404，响应不含任何 B 侧 payload
/// （F3 读断言：DB 快照对 GET 无证明力）；
/// 合法路径：userA 读自己项目的通知 → 200 且带数据。
#[sqlx::test(migrations = "../../migrations")]
async fn p0_5_notifications_cross_tenant_rejected(pool: PgPool) {
    let (a, b) = dual_tenants(&pool).await;
    let secret = "tenant-b-secret-payload";
    seed_notification(&pool, b.project_id, json!({ "secret": secret })).await;

    let uri = format!("/api/v1/projects/{}/notifications?limit=10", b.project_id);
    let response = a
        .app
        .clone()
        .oneshot(req_json("GET", &uri, Some(&a.token), None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let raw = status_text(response).await.1;
    assert!(
        !raw.contains(secret),
        "cross-tenant payload must not leak in the rejection body"
    );

    // 合法路径（同用户、自己项目）。
    seed_notification(&pool, a.project_id, json!({ "seq": 1 })).await;
    let uri = format!("/api/v1/projects/{}/notifications?limit=10", a.project_id);
    let response = a
        .app
        .clone()
        .oneshot(req_json("GET", &uri, Some(&a.token), None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = status_body(response).await.1;
    assert_eq!(body.as_array().expect("feed is an array").len(), 1);
}
