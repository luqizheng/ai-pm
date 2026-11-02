//! T-088 `/v1/pm/devices*` REST 契约 + `/mcp` 设备声明层（C176 / C184 的 HTTP 侧）。
//!
//! 覆盖：登记（201/200-reused）、他人设备与未知设备 404、绑定校验（仓库归属 /
//! 非成员 404）、自检 readiness、撤销、`attach_device_claim` 的「仅 tools/call
//! 查询与忽略审计」（C184）与四类非法声明（C176 的 HTTP 侧；完整矩阵在
//! `crates/pm-mcp/tests/caller_ctx.rs`）。
//!
//! 夹具沿用 `task_dependencies.rs` 的种子 harness（tenant external_key = JWT tid）。

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
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
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

async fn seed_repo(pool: &PgPool, project_id: Uuid, url: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, tenant_id) \
         VALUES ($1, 'r', $2, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(url)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn ignored_audit_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM governance.tenant_audit_log \
          WHERE action = 'device_attribution_ignored'",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn mcp_post(app: axum::Router, token: &str, body: Value, device: Option<&str>) -> StatusCode {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("authorization", format!("Bearer {token}"))
        .header("accept", "application/json, text/event-stream")
        .header("content-type", "application/json")
        .header("host", "localhost");
    if let Some(d) = device {
        builder = builder.header("x-ai-pm-device-id", d);
    }
    let response = app.oneshot(builder.body(Body::from(body.to_string())).unwrap()).await.unwrap();
    let status = response.status();
    if status != StatusCode::OK {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        eprintln!("mcp_post {status}: {}", String::from_utf8_lossy(&bytes));
    }
    status
}

// ── REST 契约 ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn register_reuse_and_server_generated_id(pool: PgPool) {
    let pool = pool.clone();
    let tenant = seed_tenant(&pool, "1").await;
    seed_member(&pool, tenant, "alice").await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    // 带客户端 id 首次登记 → 201
    let id = Uuid::new_v4();
    let (status, v) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                "/v1/pm/devices",
                &token,
                Some(json!({
                    "device_id": id, "name": "PC", "platform": "windows",
                    "os_version": "11", "executor_kind": "codex", "executor_version": "0.163"
                })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["id"], json!(id));
    assert_eq!(v["reused"], json!(false));
    assert_eq!(v["status"], json!("active"));

    // 同 id 再登记 → 200 + reused
    let (status, v) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                "/v1/pm/devices",
                &token,
                Some(json!({"device_id": id, "name": "PC", "platform": "windows"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["reused"], json!(true));

    // 无 id → 201 + 服务端生成
    let (status, v) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                "/v1/pm/devices",
                &token,
                Some(json!({"name": "Laptop", "platform": "macos"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert!(v["id"].is_string());

    // 平台非法 → 400
    let (status, _) = body_json(
        app.oneshot(request(
            "POST",
            "/v1/pm/devices",
            &token,
            Some(json!({"name": "X", "platform": "templeos"})),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn detail_visibility_and_404(pool: PgPool) {
    let pool = pool.clone();
    let tenant = seed_tenant(&pool, "1").await;
    seed_member(&pool, tenant, "alice").await;
    seed_member(&pool, tenant, "bob").await;
    let app = app(pool.clone());
    let alice = token_for("alice", 1);
    let bob = token_for("bob", 1);

    let (_, v) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                "/v1/pm/devices",
                &alice,
                Some(json!({"name": "PC", "platform": "windows"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let device_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();

    // bob 看 alice 的设备 → 404
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "GET",
                &format!("/v1/pm/devices/{device_id}"),
                &bob,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // alice 自己看 → 200，含 bindings 数组
    let (status, v) = body_json(
        app.clone()
            .oneshot(request("GET", &format!("/v1/pm/devices/{device_id}"), &alice, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(v["bindings"].is_array());

    // 未知设备 → 404
    let (status, _) = body_json(
        app.oneshot(request("GET", &format!("/v1/pm/devices/{}", Uuid::new_v4()), &alice, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn bind_checks_revoke_flow(pool: PgPool) {
    let pool = pool.clone();
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let customer = seed_customer(&pool, tenant, "c1").await;
    let project = seed_project(&pool, tenant, customer, "DV-API", alice).await;
    let repo = seed_repo(&pool, project, "git@x/a.git").await;
    let other_project = seed_project(&pool, tenant, customer, "DV-API-B", alice).await;
    let foreign_repo = seed_repo(&pool, other_project, "git@x/b.git").await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    let (_, v) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                "/v1/pm/devices",
                &token,
                Some(json!({"name": "PC", "platform": "windows"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let device_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();

    // 他项目的仓库 → 400（服务端校验项目归属）
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/devices/{device_id}/bindings"),
                &token,
                Some(json!({"project_id": project, "declared_repository_ids": [foreign_repo]})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 非成员项目 → 404（bob 不是任何项目成员）
    let bob = token_for("bob", 1);
    seed_member(&pool, tenant, "bob").await;
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/devices/{device_id}/bindings"),
                &bob,
                Some(json!({"project_id": project})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "bob 不是项目成员");

    // 合法绑定 + 自检到 ready
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/devices/{device_id}/bindings"),
                &token,
                Some(json!({"project_id": project, "declared_repository_ids": [repo]})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, v) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/devices/{device_id}/checks"),
                &token,
                Some(json!({
                    "installed": true, "connected": true, "tool": true,
                    "accessible_repositories": [repo]
                })),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["readiness"], json!("ready"));

    // detail 含派生的 project_accessible
    let (_, v) = body_json(
        app.clone()
            .oneshot(request("GET", &format!("/v1/pm/devices/{device_id}"), &token, None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(v["bindings"][0]["project_accessible"], json!(true));
    assert_eq!(
        v["bindings"][0]["verified_repositories"],
        json!([repo]),
        "verified_repositories 只含服务端确认的仓库"
    );

    // 撤销 → revoked；再自检 → 400
    let (status, v) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/devices/{device_id}/revoke"),
                &token,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], json!("revoked"));

    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                &format!("/v1/pm/devices/{device_id}/checks"),
                &token,
                Some(json!({"installed": true})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 解绑 → 204；再解绑 → 404
    let binding_id: Uuid = {
        let (_, v) = body_json(
            app.clone()
                .oneshot(request("GET", &format!("/v1/pm/devices/{device_id}"), &token, None))
                .await
                .unwrap(),
        )
        .await;
        // 撤销不删绑定行；detail 对 revoked 设备仍可见（owner）
        v["bindings"][0]["id"].as_str().unwrap().parse().unwrap()
    };
    let (status, _) = body_json(
        app.clone()
            .oneshot(request(
                "DELETE",
                &format!("/v1/pm/devices/{device_id}/bindings/{binding_id}"),
                &token,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = body_json(
        app.oneshot(request(
            "DELETE",
            &format!("/v1/pm/devices/{device_id}/bindings/{binding_id}"),
            &token,
            None,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── /mcp 设备声明层：C176 / C184 的 HTTP 侧 ─────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn c184_only_tools_call_triggers_device_layer(pool: PgPool) {
    let pool = pool.clone();
    let tenant = seed_tenant(&pool, "1").await;
    seed_member(&pool, tenant, "alice").await;
    let app = app(pool.clone());
    let token = token_for("alice", 1);

    assert_eq!(ignored_audit_count(&pool).await, 0);

    // initialize（带非法设备头）→ 不产生忽略审计
    let status = mcp_post(
        app.clone(),
        &token,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "t", "version": "0"}
            }
        }),
        Some("not-a-uuid"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "initialize 应到达 rmcp");
    assert_eq!(ignored_audit_count(&pool).await, 0, "非 tools/call 不写忽略审计");

    // tools/call（非法设备头）→ 写一条忽略审计（工具本身报 not found，但 HTTP 200）
    let status = mcp_post(
        app.clone(),
        &token,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "pm_no_such_tool", "arguments": {}}
        }),
        Some("not-a-uuid"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "设备声明非法不影响请求");
    assert_eq!(ignored_audit_count(&pool).await, 1, "tools/call 写一条忽略审计");

    // 再来一次 tools/call（头缺席）→ 不写（缺席不算忽略事件）
    let _ = mcp_post(
        app.clone(),
        &token,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "pm_no_such_tool", "arguments": {}}
        }),
        None,
    )
    .await;
    assert_eq!(ignored_audit_count(&pool).await, 1, "缺席头不产生忽略审计");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c176_http_side_ignored_variants_and_valid_claim(pool: PgPool) {
    let pool = pool.clone();
    let tenant = seed_tenant(&pool, "1").await;
    let alice = seed_member(&pool, tenant, "alice").await;
    let bob = seed_member(&pool, tenant, "bob").await;
    let app = app(pool.clone());
    let alice_token = token_for("alice", 1);
    let bob_token = token_for("bob", 1);

    // alice 登记一台设备
    let (_, v) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                "/v1/pm/devices",
                &alice_token,
                Some(json!({"name": "PC", "platform": "windows"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let device_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();

    let call = |token: &str, device: Option<String>| {
        let app = app.clone();
        let token = token.to_string();
        async move {
            mcp_post(
                app,
                &token,
                json!({
                    "jsonrpc": "2.0", "id": 9, "method": "tools/call",
                    "params": {"name": "pm_no_such_tool", "arguments": {}}
                }),
                device.as_deref(),
            )
            .await
        }
    };

    // unknown / not-owner / revoked / valid 各触发一次 tools/call
    let _ = call(&alice_token, Some(Uuid::new_v4().to_string())).await; // not_found
    let _ = call(&bob_token, Some(device_id.to_string())).await; // not_owner（bob 调 alice 的设备）

    // revoke 一台后用旧 id（revoked）
    let (_, v2) = body_json(
        app.clone()
            .oneshot(request(
                "POST",
                "/v1/pm/devices",
                &alice_token,
                Some(json!({"name": "Old", "platform": "linux"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let old_id: Uuid = v2["id"].as_str().unwrap().parse().unwrap();
    let _ = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/v1/pm/devices/{old_id}/revoke"),
            &alice_token,
            None,
        ))
        .await
        .unwrap();
    let _ = call(&alice_token, Some(old_id.to_string())).await; // revoked

    // valid：alice 用自己的设备
    let _ = call(&alice_token, Some(device_id.to_string())).await;

    let rows: Vec<(Option<Uuid>, serde_json::Value)> = sqlx::query_as(
        "SELECT claimed_device_id, after_data FROM governance.tenant_audit_log \
          WHERE action = 'device_attribution_ignored' ORDER BY created_at",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let reasons: Vec<&str> = rows
        .iter()
        .map(|(_, after)| after["reason"].as_str().unwrap())
        .collect();
    assert_eq!(
        reasons,
        vec!["not_found", "not_owner", "revoked"],
        "四种非法声明中缺席已单测；此处按序 not_found/not_owner/revoked，valid 不写"
    );
    assert!(
        rows.iter().all(|(id, _)| id.is_none()),
        "忽略事件的 claimed_device_id 为 NULL"
    );
}
