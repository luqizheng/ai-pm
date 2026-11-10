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

// ── Batch G：前向游标（after/after_id 增量轮询）──────────────────────────

async fn list_messages_query(
    app: axum::Router,
    token: &str,
    project_id: Uuid,
    conversation_id: Uuid,
    query: &str,
) -> (StatusCode, Value) {
    body_json(
        app.oneshot(request(
            "GET",
            &format!(
                "/v1/pm/projects/{project_id}/conversations/{conversation_id}/messages{query}"
            ),
            token,
            None,
        ))
        .await
        .unwrap(),
    )
    .await
}

/// 用例 6：`after(+after_id)` 前向增量——以首条消息为游标，只取其后消息；
/// 无游标时全量 ASC。这是前端 4s 轮询的契约面。
#[sqlx::test(migrations = "../../migrations")]
async fn after_cursor_returns_only_newer_messages(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool);
    let token = token_for("alice", 1);

    let (_, v) = create_conversation(app.clone(), &token, project).await;
    let conversation_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();
    let (_, first) = post_message(
        app.clone(),
        &token,
        project,
        conversation_id,
        json!({"content": "first"}),
    )
    .await;
    let (_, second) = post_message(
        app.clone(),
        &token,
        project,
        conversation_id,
        json!({"content": "second"}),
    )
    .await;

    let query = format!(
        "?after={}&after_id={}",
        first["created_at"].as_str().unwrap(),
        first["id"].as_str().unwrap()
    );
    let (status, v) =
        list_messages_query(app.clone(), &token, project, conversation_id, &query).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let items = v.as_array().unwrap();
    assert_eq!(items.len(), 1, "{v}");
    assert_eq!(items[0]["content"], json!("second"));
    assert_eq!(items[0]["id"], second["id"]);

    // 以最后一条为游标 → 无增量（幂等轮询面）。
    let query = format!(
        "?after={}&after_id={}",
        second["created_at"].as_str().unwrap(),
        second["id"].as_str().unwrap()
    );
    let (status, v) = list_messages_query(app, &token, project, conversation_id, &query).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v.as_array().map(Vec::len), Some(0), "{v}");
}

/// 用例 7：游标组合校验——before/after 同给 400；after_id 缺 after 400；
/// 非法 after（非 RFC3339）400；非法 after_id（非 UUID）400。
#[sqlx::test(migrations = "../../migrations")]
async fn cursor_combinations_are_validated(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool);
    let token = token_for("alice", 1);

    let (_, v) = create_conversation(app.clone(), &token, project).await;
    let conversation_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();
    let _base = format!("/v1/pm/projects/{project}/conversations/{conversation_id}/messages");

    // before + after 同给 → 400。
    let (status, _) = list_messages_query(
        app.clone(),
        &token,
        project,
        conversation_id,
        "?before=2026-11-08T10:00:00Z&after=2026-11-08T10:00:00Z",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // after_id 缺 after → 400。
    let (status, _) = list_messages_query(
        app.clone(),
        &token,
        project,
        conversation_id,
        "?after_id=0f0e0d0c-0b0a-4938-8111-111111111111",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 非法 after（非 RFC3339）→ 400。
    let (status, _) = list_messages_query(
        app.clone(),
        &token,
        project,
        conversation_id,
        "?after=not-a-time",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 非法 after_id（非 UUID）→ 400。
    let (status, _) = list_messages_query(
        app,
        &token,
        project,
        conversation_id,
        "?after=2026-11-08T10:00:00Z&after_id=not-a-uuid",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Batch H：latest 首屏「最新窗口」─────────────────────────────────────

/// 用例 8：`latest=true&limit=N` 返回**最新** N 条、顺序仍 ASC（首屏不再从
/// 最旧窗口起）；消息数 ≤ limit 时即全量。latest 与 before / after 任一同给
/// → 400（从简：组合语义不做）。
#[sqlx::test(migrations = "../../migrations")]
async fn latest_query_returns_newest_window_and_conflicts_are_400(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let app = app(pool);
    let token = token_for("alice", 1);

    let (_, v) = create_conversation(app.clone(), &token, project).await;
    let conversation_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();
    let mut contents = Vec::new();
    for i in 0..4 {
        let (_, m) = post_message(
            app.clone(),
            &token,
            project,
            conversation_id,
            json!({"content": format!("m{i}")}),
        )
        .await;
        contents.push(m);
    }

    // latest=true&limit=2 → 最新 2 条（m2,m3），ASC。
    let (status, v) = list_messages_query(
        app.clone(),
        &token,
        project,
        conversation_id,
        "?latest=true&limit=2",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let items = v.as_array().unwrap();
    assert_eq!(items.len(), 2, "{v}");
    assert_eq!(items[0]["content"], json!("m2"));
    assert_eq!(items[1]["content"], json!("m3"));
    assert_eq!(items[1]["id"], contents[3]["id"]);

    // ≤ limit：全量 ASC（latest 与默认同形）。
    let (status, v) = list_messages_query(
        app.clone(),
        &token,
        project,
        conversation_id,
        "?latest=true",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v.as_array().map(Vec::len), Some(4), "{v}");

    // latest + before 同给 → 400。
    let (status, _) = list_messages_query(
        app.clone(),
        &token,
        project,
        conversation_id,
        &format!(
            "?latest=true&before={}",
            contents[0]["created_at"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // latest + after 同给 → 400。
    let (status, _) = list_messages_query(
        app,
        &token,
        project,
        conversation_id,
        &format!(
            "?latest=true&after={}",
            contents[0]["created_at"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Batch 2：线程级上下文锚点（0003）──────────────────────────────────

/// 建会话（可带 context 锚点）。
async fn create_conversation_with(
    app: axum::Router,
    token: &str,
    project_id: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    body_json(
        app.oneshot(request(
            "POST",
            &format!("/v1/pm/projects/{project_id}/conversations"),
            token,
            Some(body),
        ))
        .await
        .unwrap(),
    )
    .await
}

/// 播种本项目真实仓库（锚点真实性校验的对照物）。
async fn seed_repository(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    name: &str,
    default_branch: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
             (tenant_id, project_id, name, url, default_branch, status) \
         VALUES ($1, $2, $3, $4, $5, 'verified') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(name)
    .bind(format!("https://example.invalid/{name}.git"))
    .bind(default_branch)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A-4：合法锚点落库；缺省 context → 锚点为 null（存量兼容）。
#[sqlx::test(migrations = "../../migrations")]
async fn context_anchor_is_stored_on_creation(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    seed_repository(&pool, tenant, project, "atlas-platform", "main").await;
    let app = app(pool);
    let token = token_for("alice", 1);

    // 带锚点建会话 → 201 且锚点回显。
    let (status, v) = create_conversation_with(
        app.clone(),
        &token,
        project,
        json!({
            "title": "带上下文",
            "context": {
                "project_id": project.to_string(),
                "project_name": "p1",
                "repository_name": "atlas-platform",
                "default_branch": "main",
                "repository_role": "main"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(
        v["context_anchor"]["repository_name"],
        json!("atlas-platform")
    );
    assert_eq!(v["context_anchor"]["default_branch"], json!("main"));

    // 缺省 context → 锚点为 null（存量会话兼容）。
    let (status, v) = create_conversation(app.clone(), &token, project).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert!(v["context_anchor"].is_null(), "{v}");

    // 列表回读同样带锚点。
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
    let items = v.as_array().expect("array");
    let anchored = items
        .iter()
        .find(|item| item["title"] == json!("带上下文"))
        .expect("anchored conversation");
    assert_eq!(
        anchored["context_anchor"]["repository_name"],
        json!("atlas-platform")
    );
}

/// A-5：白名单外字段 / 不存在的仓库 / 跨项目锚点 / 非法分支 → 400。
#[sqlx::test(migrations = "../../migrations")]
async fn context_anchor_rejects_invalid_values(pool: PgPool) {
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "p1", alice).await;
    let other_project = seed_project(&pool, tenant, customer, "p2", alice).await;
    seed_repository(&pool, tenant, project, "atlas-platform", "main").await;
    let app = app(pool);
    let token = token_for("alice", 1);

    // 白名单外字段 → 422（axum Json 拒绝器：body 无法反序列化成契约结构）。
    // 与 400 区分：400 = 结构合法但语义越界（仓库不存在 / 跨项目锚点）。
    let (status, _) = create_conversation_with(
        app.clone(),
        &token,
        project,
        json!({
            "context": {
                "project_id": project.to_string(),
                "secret_token": "leak"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // 仓库不存在（本项目内）→ 400。
    let (status, _) = create_conversation_with(
        app.clone(),
        &token,
        project,
        json!({
            "context": {
                "project_id": project.to_string(),
                "repository_name": "no-such-repo"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 锚点 project_id 与目标项目不符 → 400。
    let (status, _) = create_conversation_with(
        app.clone(),
        &token,
        project,
        json!({
            "context": {
                "project_id": other_project.to_string(),
                "repository_name": "atlas-platform"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 分支不属于该仓库 → 400。
    let (status, _) = create_conversation_with(
        app.clone(),
        &token,
        project,
        json!({
            "context": {
                "project_id": project.to_string(),
                "repository_name": "atlas-platform",
                "default_branch": "release/9999"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
