//! Batch 3C `/v1/pm/projects/{pid}/conversations/{cid}/messages/{mid}/interaction`
//! 交互写回契约：终态落库 + 信封回读 + 生命周期（幂等/单向/权限）。
//!
//! 覆盖整合计划验收项 A-16~A-22（四反馈语义 / 幂等单向 / 回读持久化 / 权限）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
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

fn token_for(sub: &str, tid: i64) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": sub, "tid": tid, "username": sub, "roles": [],
            "permissions": ["PM_MCP_READ"], "aud": "coder-micro-services",
        }),
        Duration::from_secs(300),
    )
    .expect("test token must sign")
}

fn request(method: &str, uri: &str, token: &str, body: Option<Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");
    match body {
        Some(v) => builder.body(Body::from(v.to_string())).unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

fn app(pool: PgPool) -> axum::Router {
    router_with_auth_config(pool, test_jwks(), AuthConfig::default())
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let (status, body) = (response.status(), response.into_body());
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

async fn seed_tenant(pool: &PgPool, external_key: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', $1, $1) RETURNING id",
    )
    .bind(external_key)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_member(pool: &PgPool, tenant_id: Uuid, sub: &str) -> Uuid {
    let principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', $1, true) RETURNING id",
    )
    .bind(sub)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3)",
    )
    .bind(tenant_id)
    .bind(sub)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, 'member')",
    )
    .bind(tenant_id)
    .bind(principal)
    .execute(pool)
    .await
    .unwrap();
    principal
}

async fn seed_customer(pool: &PgPool, tenant_id: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, code, name) VALUES ($1, $2, $2) RETURNING id",
    )
    .bind(tenant_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_project(
    pool: &PgPool,
    tenant_id: Uuid,
    customer_id: Uuid,
    code: &str,
    member: Uuid,
) -> Uuid {
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(member)
    .execute(pool)
    .await
    .unwrap();
    project_id
}

async fn create_conversation(app: axum::Router, token: &str, project_id: Uuid) -> Uuid {
    let (status, v) = body_json(
        app.oneshot(request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/conversations"),
            token,
            Some(json!({})),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    Uuid::parse_str(v["id"].as_str().unwrap()).unwrap()
}

/// 播一条带信封的 assistant 消息（交互靶子）。
async fn seed_card_message(
    pool: &PgPool,
    conversation_id: Uuid,
    envelope_status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO ai.conversation_message \
             (conversation_id, role, content, content_type, metadata) \
         VALUES ($1, 'assistant', '卡片消息', 'markdown', \
                 jsonb_build_object('envelope', jsonb_build_object( \
                     'id', 'env-1', 'type', 'recommendation', 'status', $2::text, \
                     'identity', jsonb_build_object('name', 'AI-PM'), \
                     'createdAt', '2026-11-09T00:00:00Z', 'title', '建议', \
                     'payload', jsonb_build_object( \
                         'summary', 's', 'explanationMarkdown', 'e', \
                         'benefits', jsonb_build_array(), 'risks', jsonb_build_array())))) \
         RETURNING id",
    )
    .bind(conversation_id)
    .bind(envelope_status)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn submit_interaction(
    app: axum::Router,
    token: &str,
    project_id: Uuid,
    conversation_id: Uuid,
    message_id: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    body_json(
        app.oneshot(request(
            "POST",
            &format!(
                "/v1/pm/projects/{project_id}/conversations/{conversation_id}/\
                 messages/{message_id}/interaction"
            ),
            token,
            Some(body),
        ))
        .await
        .unwrap(),
    )
    .await
}

/// A-16 接受反馈：accept → 201 accepted + 记录号；信封回读终态 + result。
#[sqlx::test(migrations = "../../migrations")]
async fn accept_records_terminal_and_updates_envelope(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    let conversation_id = create_conversation(app.clone(), &token, project).await;
    let message_id = seed_card_message(&pool, conversation_id, "presented").await;

    let (status, v) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "accept"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["status"], json!("accepted"));
    assert_eq!(v["action"], json!("accept"));
    assert_eq!(v["decided_by"], json!(alice));
    let reference = v["reference"].as_str().unwrap();
    assert!(reference.starts_with("INT-"), "{reference}");
    assert_eq!(v["replayed"], json!(false));

    // A-21 回读：消息信封终态化 + 记录号结果。
    let (envelope_status, result): (String, Value) = sqlx::query_as(
        "SELECT metadata->'envelope'->>'status', metadata->'envelope'->'result' \
         FROM ai.conversation_message WHERE id = $1",
    )
    .bind(message_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(envelope_status, "accepted");
    assert_eq!(result["recorded"], json!(true));
    assert_eq!(result["reference"], json!(reference));
}

/// A-17 拒绝反馈：reject 带原因 → rejected；原因落库且回读可见；缺原因 400。
#[sqlx::test(migrations = "../../migrations")]
async fn reject_requires_reason_and_persists_it(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    let conversation_id = create_conversation(app.clone(), &token, project).await;
    let message_id = seed_card_message(&pool, conversation_id, "presented").await;

    // 缺原因 → 400。
    let (status, _) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "reject"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 带原因 → 201 rejected + 原因落库。
    let (status, v) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "reject", "reason": "风险不可接受"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["status"], json!("rejected"));
    assert_eq!(v["reason"], json!("风险不可接受"));
}

/// A-18 提交/决策反馈：decision 未勾本人确认 400（服务端复验）；确认后 resolved。
#[sqlx::test(migrations = "../../migrations")]
async fn decision_requires_confirmation_and_option(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    let conversation_id = create_conversation(app.clone(), &token, project).await;
    let message_id = seed_card_message(&pool, conversation_id, "presented").await;

    // 未勾本人确认 → 400。
    let (status, _) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "decision", "option_id": "opt-1"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 勾了确认但缺 option → 400。
    let (status, _) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "decision", "answer": {"confirmed": true}}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 确认 + option → 201 resolved。
    let (status, v) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "decision", "option_id": "opt-1",
               "answer": {"confirmed": true, "comment": "按方案 A 执行"}}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["status"], json!("resolved"));
    assert_eq!(v["option_id"], json!("opt-1"));
}

/// A-19 失败反馈：未知动作 400；纯文本消息（无信封）不可交互 400。
#[sqlx::test(migrations = "../../migrations")]
async fn invalid_action_and_plain_message_are_rejected(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    let conversation_id = create_conversation(app.clone(), &token, project).await;
    let message_id = seed_card_message(&pool, conversation_id, "presented").await;

    // 未知动作 → 400。
    let (status, _) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "explode"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 纯文本消息（无信封）→ 400 not interactive。
    let plain_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.conversation_message (conversation_id, role, content) \
         VALUES ($1, 'assistant', '纯文本') RETURNING id",
    )
    .bind(conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, _) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        plain_id,
        json!({"action": "accept"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// A-20 幂等与单向：同 action 重提交 200 既有记录（replayed）；换 action 409。
#[sqlx::test(migrations = "../../migrations")]
async fn terminal_is_idempotent_for_same_action_and_conflicts_on_change(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    let conversation_id = create_conversation(app.clone(), &token, project).await;
    let message_id = seed_card_message(&pool, conversation_id, "presented").await;

    let (status, v1) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "accept"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // 同 action 幂等 → 200 + replayed=true + 同一记录号。
    let (status, v2) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "accept"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v2["replayed"], json!(true));
    assert_eq!(v2["reference"], v1["reference"]);

    // 换 action → 409。
    let (status, _) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "reject", "reason": "反悔"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

/// A-21 回读持久化（幂等之外的另一面）：信封终态化后 GET messages 带终态与结果。
#[sqlx::test(migrations = "../../migrations")]
async fn envelope_readback_visible_via_messages_list(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    let conversation_id = create_conversation(app.clone(), &token, project).await;
    let message_id = seed_card_message(&pool, conversation_id, "presented").await;

    let (_, v) = submit_interaction(
        app.clone(),
        &token,
        project,
        conversation_id,
        message_id,
        json!({"action": "accept"}),
    )
    .await;
    let reference = v["reference"].as_str().unwrap().to_string();

    let (status, items) = body_json(
        app.oneshot(request(
            "GET",
            &format!(
                "/v1/pm/projects/{project}/conversations/{conversation_id}/messages?latest=true&limit=50"
            ),
            &token,
            None,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let card = items
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == json!(message_id.to_string()))
        .expect("card message listed");
    assert_eq!(card["metadata"]["envelope"]["status"], json!("accepted"));
    assert_eq!(
        card["metadata"]["envelope"]["result"]["reference"],
        json!(reference)
    );
    assert_eq!(
        card["metadata"]["envelope"]["result"]["recorded"],
        json!(true)
    );
}

/// A-22 权限：非属主（同项目另一成员）对他人会话交互 → 404 同形防探测。
#[sqlx::test(migrations = "../../migrations")]
async fn cross_owner_interaction_is_404(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let bob = seed_member(&pool, tenant, "bob").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'member')",
    )
    .bind(tenant)
    .bind(project)
    .bind(bob)
    .execute(&pool)
    .await
    .unwrap();
    let app = app(pool.clone());
    let alice_token = token_for("alice", 1);
    let bob_token = token_for("bob", 1);

    let conversation_id = create_conversation(app.clone(), &alice_token, project).await;
    let message_id = seed_card_message(&pool, conversation_id, "presented").await;

    let (status, _) = submit_interaction(
        app.clone(),
        &bob_token,
        project,
        conversation_id,
        message_id,
        json!({"action": "accept"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
