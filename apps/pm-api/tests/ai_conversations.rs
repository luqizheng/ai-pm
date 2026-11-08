//! V2 D-4=A `/v1/pm/projects/{project_id}/conversations*` REST 契约。
//!
//! 覆盖：建会话（标题默认空）、消息追加（ASC / last_message_at 刷新 /
//! role 恒 user）、属主隔离（同项目另一成员 404）、不可见项目 404、
//! 空 content 400。夹具沿用 `pm_devices.rs` 的种子 harness。

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
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn app(pool: PgPool) -> axum::Router {
    router_with_auth_config(pool, test_jwks(), AuthConfig::default())
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
        "INSERT INTO core.customer (tenant_id, code, name, status) VALUES ($1, $2, $2, 'active') RETURNING id",
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

/// 追加项目成员（非 owner 角色）：属主隔离用例的另一成员。
async fn seed_project_member(pool: &PgPool, tenant_id: Uuid, project_id: Uuid, member: Uuid) {
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'member')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(member)
    .execute(pool)
    .await
    .unwrap();
}

async fn create_conversation(
    app: axum::Router,
    token: &str,
    project_id: Uuid,
) -> (StatusCode, Value) {
    body_json(
        app.oneshot(request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/conversations"),
            token,
            Some(json!({})),
        ))
        .await
        .unwrap(),
    )
    .await
}

async fn post_message(
    app: axum::Router,
    token: &str,
    project_id: Uuid,
    conversation_id: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    body_json(
        app.oneshot(request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/conversations/{conversation_id}/messages"),
            token,
            Some(body),
        ))
        .await
        .unwrap(),
    )
    .await
}

// ── 契约 ─────────────────────────────────────────────────────────────

/// 用例 1：建会话 → 列表可见，标题默认空。
#[sqlx::test(migrations = "../../migrations")]
async fn create_conversation_defaults_and_lists(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool);
    let token = token_for("alice", 1);

    let (status, v) = create_conversation(app.clone(), &token, project).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["title"], json!(""));
    assert_eq!(v["status"], json!("active"));
    assert_eq!(v["principal_id"], json!(alice));
    assert_eq!(v["project_id"], json!(project));
    assert!(v["last_message_at"].is_null(), "{v}");
    let conversation_id = v["id"].clone();

    // 列表可见（仅本人），标题默认空。
    let (status, v) = body_json(
        app.oneshot(request(
            "GET",
            &format!("/v1/pm/projects/{project}/conversations"),
            &token,
            None,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v.as_array().map(Vec::len), Some(1), "{v}");
    assert_eq!(v[0]["id"], conversation_id);
    assert_eq!(v[0]["title"], json!(""));
}

/// 用例 2：追加两条 user 消息 → ASC、content_type 默认、role 恒 user
/// （body 里塞 role 被忽略）、last_message_at 跟随最后一条刷新。
#[sqlx::test(migrations = "../../migrations")]
async fn append_messages_order_and_last_message_at(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool);
    let token = token_for("alice", 1);

    let (_, v) = create_conversation(app.clone(), &token, project).await;
    let conversation_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();

    let (status, v) = post_message(
        app.clone(),
        &token,
        project,
        conversation_id,
        json!({"content": "first"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["role"], json!("user"));
    assert_eq!(v["content_type"], json!("markdown"));
    assert_eq!(v["metadata"], json!({}));

    // body 携带 role / metadata 也被忽略：REST 恒 user、元数据不外收。
    let (status, v2) = post_message(
        app.clone(),
        &token,
        project,
        conversation_id,
        json!({"content": "second", "content_type": "text", "role": "assistant",
               "metadata": {"injected": true}}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v2}");
    assert_eq!(v2["role"], json!("user"));
    assert_eq!(v2["content_type"], json!("text"));
    assert_eq!(v2["metadata"], json!({}));

    // 消息按 created_at ASC。
    let (status, v) = body_json(
        app.clone()
            .oneshot(request(
                "GET",
                &format!("/v1/pm/projects/{project}/conversations/{conversation_id}/messages"),
                &token,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v.as_array().map(Vec::len), Some(2), "{v}");
    assert_eq!(v[0]["content"], json!("first"));
    assert_eq!(v[1]["content"], json!("second"));

    // last_message_at 已刷新为最后一条消息时间（now() 同事务 → 精确相等）。
    let (status, v) = body_json(
        app.oneshot(request(
            "GET",
            &format!("/v1/pm/projects/{project}/conversations"),
            &token,
            None,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v[0]["last_message_at"], v2["created_at"], "{v}");
}

/// 用例 3：同项目另一成员访问他人会话 → 404（属主隔离，防探测）；
/// 对方自己的会话列表为空。
#[sqlx::test(migrations = "../../migrations")]
async fn cross_owner_same_project_is_404(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let bob = seed_member(&pool, tenant, "bob").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    seed_project_member(&pool, tenant, project, bob).await;
    let app = app(pool);
    let alice_token = token_for("alice", 1);
    let bob_token = token_for("bob", 1);

    let (_, v) = create_conversation(app.clone(), &alice_token, project).await;
    let conversation_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();
    let (status, _) = post_message(
        app.clone(),
        &alice_token,
        project,
        conversation_id,
        json!({"content": "private"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // bob 能看见项目，但读 / 写他人会话一律 404。
    let (status, v) = body_json(
        app.clone()
            .oneshot(request(
                "GET",
                &format!("/v1/pm/projects/{project}/conversations/{conversation_id}/messages"),
                &bob_token,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");

    let (status, v) = post_message(
        app.clone(),
        &bob_token,
        project,
        conversation_id,
        json!({"content": "intrude"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");

    // bob 自己的会话列表为空（列表按属主过滤，不 404）。
    let (status, v) = body_json(
        app.oneshot(request(
            "GET",
            &format!("/v1/pm/projects/{project}/conversations"),
            &bob_token,
            None,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v.as_array().map(Vec::len), Some(0), "{v}");
}

/// 用例 4：不可见项目（同租户非成员）→ 404，不泄露存在性。
#[sqlx::test(migrations = "../../migrations")]
async fn invisible_project_is_404(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let bob = seed_member(&pool, tenant, "bob").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    // p1 归 alice，p2 只归 bob：alice 对 p2 不可见。
    seed_project(&pool, tenant, customer, "p1", alice).await;
    let hidden_project = seed_project(&pool, tenant, customer, "p2", bob).await;
    let app = app(pool);
    let alice_token = token_for("alice", 1);

    let (status, v) = create_conversation(app.clone(), &alice_token, hidden_project).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");

    let (status, v) = body_json(
        app.clone()
            .oneshot(request(
                "GET",
                &format!("/v1/pm/projects/{hidden_project}/conversations"),
                &alice_token,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");

    // 未知 conversation_id 在可见项目里同样 404。
    let unknown = Uuid::new_v4();
    let (status, _) = post_message(
        app,
        &alice_token,
        hidden_project,
        unknown,
        json!({"content": "x"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// 用例 5：content 为空 → 400 Validation；非法 before 游标 → 400。
#[sqlx::test(migrations = "../../migrations")]
async fn empty_content_is_validation_error(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool);
    let token = token_for("alice", 1);

    let (_, v) = create_conversation(app.clone(), &token, project).await;
    let conversation_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();

    let (status, v) = post_message(
        app.clone(),
        &token,
        project,
        conversation_id,
        json!({"content": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");

    // 纯空白同样拒绝。
    let (status, _) = post_message(
        app.clone(),
        &token,
        project,
        conversation_id,
        json!({"content": "   "}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 非法 before（非 RFC3339）→ 400。
    let (status, _) = body_json(
        app.oneshot(request(
            "GET",
            &format!(
                "/v1/pm/projects/{project}/conversations/{conversation_id}/messages?before=not-a-time"
            ),
            &token,
            None,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
