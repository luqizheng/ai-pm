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
//! 主体：tenantA = SYSTEM_TENANT（identity 经 lmms:1 通道映射，claims tid=1）；
//! tenantB = 独立 tenant（raw seed），仅作攻击目标。ownerB 全量双租户合法路径
//! 归 G2 阶段双租户夹具（交付物 05 §6 阶段 B）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::routes::router_with_auth;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

fn sign_user(sub: &str) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub,
            "tid": 1,
            "username": "alice",
            "roles": ["tenant_admin"],
            "permissions": []
        }),
        Duration::from_secs(300),
    )
    .expect("test token must sign")
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn read_body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn json_request(method: &str, uri: &str, token: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
        .unwrap()
}

/// tenantA：bootstrap projectA1，并把 human owner 经 lmms:1 identity 通道映射到
/// JWT sub（claims tid=1）。返回 (app, token, human_id, project_id)。
async fn fixture_tenant_a(pool: &PgPool) -> (axum::Router, String, Uuid, Uuid) {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "G1-A1".to_string(),
            project_name: "g1 tenant a".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    // tid=1 必须映射到项目所在租户（SYSTEM_TENANT）：迁移只种 id=0x10 的
    // system 行，这里把它改标为 lmms:1，使 caller.tenant_id 与项目租户一致。
    sqlx::query("UPDATE core.tenant SET provider = 'lmms', external_key = '1' WHERE id = $1")
        .bind(SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3)",
    )
    .bind(SYSTEM_TENANT)
    .bind(boot.human_id.to_string())
    .bind(boot.human_id)
    .execute(pool)
    .await
    .unwrap();
    let token = sign_user(&boot.human_id.to_string());
    (
        router_with_auth(pool.clone(), test_jwks()),
        token,
        boot.human_id,
        boot.project_id,
    )
}

/// tenantB：独立 tenant + 原始 project（无任何 identity/member 映射到 userA）。
async fn seed_tenant_b_project(pool: &PgPool) -> Uuid {
    let tenant_b = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.tenant (id, provider, external_key, display_name) \
         VALUES ($1, 'lmms', '2', 'lmms:2-g1')",
    )
    .bind(tenant_b)
    .execute(pool)
    .await
    .unwrap();
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'g1-b') RETURNING id",
    )
    .bind(tenant_b)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, 'G1-B1', 'g1 tenant b', 'planning') RETURNING id",
    )
    .bind(tenant_b)
    .bind(customer_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

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

async fn seed_ai_config(pool: &PgPool, project_id: Uuid, ai_principal: Uuid) {
    sqlx::query(
        "INSERT INTO ai.project_ai_config \
         (project_id, ai_principal_id, model_provider, model_name, automation_mode, enabled, tenant_id) \
         VALUES ($1, $2, 'test', 'fake-model', 'ai_lead', true, \
                  (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(ai_principal)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_ai_principal(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'g1-seed-ai') RETURNING id")
        .bind(Uuid::new_v4())
        .fetch_one(pool)
        .await
        .unwrap()
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
    let (app, token, _human, project_a) = fixture_tenant_a(&pool).await;
    let project_b = seed_tenant_b_project(&pool).await;

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/events",
            &token,
            Some(json!({
                "project_id": project_b,
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
    let body = body_json(response).await;
    assert_eq!(body["error"], json!("project_not_found"));

    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM integration.inbound_event), \
                (SELECT count(*) FROM integration.job)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, 0), "attack must not write event or job rows");

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/events",
            &token,
            Some(json!({
                "project_id": project_a,
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
    let (app, token, human_a, project_a) = fixture_tenant_a(&pool).await;
    let project_b = seed_tenant_b_project(&pool).await;
    let (proposal_b, approval_b) = seed_proposal_chain(&pool, project_b, "proposed").await;
    let approval_b = approval_b.expect("proposed seed yields a pending approval");

    let uri = format!("/api/v1/supervision/approvals/{approval_b}/decision");
    let response = app.clone()
        .oneshot(json_request(
            "POST",
            &uri,
            &token,
            Some(json!({ "approval_id": approval_b, "actor_id": Uuid::new_v4(), "decision": "reject", "reason": "cross tenant" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
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
            .bind(project_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(notifications, 0);

    // 合法路径（同用户、自己项目）。
    let (proposal_a, approval_a) = seed_proposal_chain(&pool, project_a, "proposed").await;
    let approval_a = approval_a.expect("proposed seed yields a pending approval");
    let uri = format!("/api/v1/supervision/approvals/{approval_a}/decision");
    let response = app.clone()
        .oneshot(json_request(
            "POST",
            &uri,
            &token,
            Some(json!({ "approval_id": approval_a, "actor_id": Uuid::new_v4(), "decision": "reject", "reason": "own project" })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
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
        Some(human_a),
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
    let (app, token, _human, project_a) = fixture_tenant_a(&pool).await;
    let project_b = seed_tenant_b_project(&pool).await;
    let (proposal_b, _) = seed_proposal_chain(&pool, project_b, "executed").await;

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/supervision/veto",
            &token,
            Some(json!({
                "project_id": project_b,
                "proposal_id": proposal_b,
                "actor_id": Uuid::new_v4(),
                "reason": "cross tenant"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["error"], json!("project_not_found"));

    let task: (String, i64) = sqlx::query_as(
        "SELECT status, revision FROM delivery.task WHERE project_id = $1 AND title = 'G1-TASK'",
    )
    .bind(project_b)
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
    .bind(project_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(interventions, 0);
    let notifications: i64 =
        sqlx::query_scalar("SELECT count(*) FROM integration.notification WHERE project_id = $1")
            .bind(project_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(notifications, 0);

    // 合法路径（同用户、自己项目）。
    let (proposal_a, _) = seed_proposal_chain(&pool, project_a, "executed").await;
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/supervision/veto",
            &token,
            Some(json!({
                "project_id": project_a,
                "proposal_id": proposal_a,
                "actor_id": Uuid::new_v4(),
                "reason": "own project rollback"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["outcome"], json!("rolled_back"));
    let interventions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.human_intervention \
         WHERE project_id = $1 AND intervention_type = 'veto'",
    )
    .bind(project_a)
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
    let (app, token, _human, project_a) = fixture_tenant_a(&pool).await;
    let project_b = seed_tenant_b_project(&pool).await;
    // projectA 的 ai.config 由 bootstrap_project 种子自带；只为 tenantB 手工播种。
    let ai_principal_b = seed_ai_principal(&pool).await;
    seed_ai_config(&pool, project_b, ai_principal_b).await;

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/supervision/freeze",
            &token,
            Some(json!({
                "project_id": project_b,
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
    .bind(project_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mode, "ai_lead", "automation_mode must stay untouched");

    // 合法路径（同用户、自己项目）。
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/supervision/freeze",
            &token,
            Some(json!({
                "project_id": project_a,
                "actor_id": Uuid::new_v4(),
                "reason": "own project brake"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["outcome"], json!("frozen"));
    let mode: String = sqlx::query_scalar(
        "SELECT automation_mode FROM ai.project_ai_config WHERE project_id = $1",
    )
    .bind(project_a)
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
    let (app, token, _human, project_a) = fixture_tenant_a(&pool).await;
    let project_b = seed_tenant_b_project(&pool).await;
    let secret = "tenant-b-secret-payload";
    seed_notification(&pool, project_b, json!({ "secret": secret })).await;

    let uri = format!("/api/v1/projects/{project_b}/notifications?limit=10");
    let response = app
        .clone()
        .oneshot(json_request("GET", &uri, &token, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let raw = read_body(response).await;
    assert!(
        !raw.contains(secret),
        "cross-tenant payload must not leak in the rejection body"
    );

    // 合法路径（同用户、自己项目）。
    seed_notification(&pool, project_a, json!({ "seq": 1 })).await;
    let uri = format!("/api/v1/projects/{project_a}/notifications?limit=10");
    let response = app
        .clone()
        .oneshot(json_request("GET", &uri, &token, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body.as_array().expect("feed is an array").len(), 1);
}
