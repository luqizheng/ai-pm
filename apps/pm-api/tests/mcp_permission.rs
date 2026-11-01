//! MCP endpoint permission gate (integration spec Phase 1 §3): every `/mcp`
//! call requires `PM_MCP_READ`, and write tools additionally require their
//! per-tool permission (`PM_MCP_DOCUMENT_WRITE`). The rest of the API is
//! unaffected by the permission claim.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use digua_common_jwt::{JWKSCache, sign_token};
use pm_api::auth::AuthConfig;
use pm_api::routes::router_with_auth_config;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

const INITIALIZE_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"gate-test","version":"0.1.0"}}}"#;

fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

fn sign(permissions: &[&str]) -> String {
    sign_token(
        PRIVATE_PEM,
        &json!({
            "sub": "usr-1",
            "tid": 1,
            "username": "alice",
            "roles": ["tenant_admin"],
            "permissions": permissions,
        }),
        Duration::from_secs(300),
    )
    .expect("test token must sign")
}

/// 本文件的 token 都是 `usr-1`；配置成首管理员，使其首次请求即激活，
/// 这样这些用例只考察权限门，不撞上 T-021a 的未激活 fail-closed。
fn app(pool: PgPool) -> axum::Router {
    router_with_auth_config(
        pool,
        test_jwks(),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-1".to_string()),
        },
    )
}

fn mcp_post(token: Option<&str>, body: String) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", "127.0.0.1:8090")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body)).unwrap()
}

fn mcp_request(token: Option<&str>) -> Request<Body> {
    mcp_post(token, INITIALIZE_BODY.to_string())
}

fn mcp_tool_call(token: Option<&str>, tool: &str, arguments: Value) -> Request<Body> {
    mcp_post(
        token,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        })
        .to_string(),
    )
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn mcp_without_token_is_unauthorized(pool: PgPool) {
    let response = app(pool)
        .oneshot(mcp_request(None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn mcp_without_mcp_read_is_forbidden(pool: PgPool) {
    let token = sign(&["IAM_SYSTEM_READ"]);
    let response = app(pool)
        .oneshot(mcp_request(Some(&token)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_json(response).await;
    assert_eq!(body["code"], json!("IAM_PERM_3001"), "forbidden code");
    assert!(body["message"].as_str().unwrap().contains("PM_MCP_READ"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn mcp_with_mcp_read_passes_the_gate(pool: PgPool) {
    let token = sign(&["PM_MCP_READ"]);
    let response = app(pool)
        .oneshot(mcp_request(Some(&token)))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "rmcp initialize must reach the service unchanged behind the gate"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_mcp_routes_ignore_the_permission_claim(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "MCP-1".to_string(),
            project_name: "mcp project".to_string(),
        },
    )
    .await
    .unwrap();
    // Permissionless token: the gate must only touch /mcp.
    let token = sign(&[]);
    let response = app(pool)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/projects/{}/notifications", boot.project_id))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
fn submit_arguments() -> Value {
    json!({
        "project_id": Uuid::new_v4(),
        "task_ref": Uuid::new_v4().to_string(),
        "idempotency_key": "tool-gate-1",
        "fixture": "design-v1",
        "summary": "gate test"
    })
}

#[sqlx::test(migrations = "../../migrations")]
async fn write_tool_without_document_write_is_forbidden(pool: PgPool) {
    // PM_MCP_READ alone must not open the document-write tool.
    let token = sign(&["PM_MCP_READ"]);
    let response = app(pool)
        .oneshot(mcp_tool_call(
            Some(&token),
            "pm_submit_technical_design",
            submit_arguments(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_json(response).await;
    assert_eq!(body["code"], json!("IAM_PERM_3001"));
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("PM_MCP_DOCUMENT_WRITE"),
        "message must name the missing tool permission: {body}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn write_tool_with_document_write_passes_the_gate(pool: PgPool) {
    let token = sign(&["PM_MCP_READ", "PM_MCP_DOCUMENT_WRITE"]);
    let response = app(pool)
        .oneshot(mcp_tool_call(
            Some(&token),
            "pm_submit_technical_design",
            submit_arguments(),
        ))
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::FORBIDDEN);
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    // The tool itself may answer isError (unknown project), but the gate is open.
}

#[sqlx::test(migrations = "../../migrations")]
async fn read_tool_needs_only_mcp_read(pool: PgPool) {
    let token = sign(&["PM_MCP_READ"]);
    let response = app(pool)
        .oneshot(mcp_tool_call(
            Some(&token),
            "pm_get_task_context",
            json!({ "project_id": Uuid::new_v4(), "task_ref": Uuid::new_v4().to_string() }),
        ))
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::FORBIDDEN);
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn write_permission_does_not_replace_mcp_read(pool: PgPool) {
    // Endpoint gate still applies first: no PM_MCP_READ → 403 even for initialize.
    let token = sign(&["PM_MCP_DOCUMENT_WRITE"]);
    let response = app(pool)
        .oneshot(mcp_request(Some(&token)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_json(response).await;
    assert!(body["message"].as_str().unwrap().contains("PM_MCP_READ"));
}
#[sqlx::test(migrations = "../../migrations")]
async fn document_write_tool_requires_document_write(pool: PgPool) {
    let token = sign(&["PM_MCP_READ"]);
    let response = app(pool)
        .oneshot(mcp_tool_call(
            Some(&token),
            "pm_submit_document",
            json!({
                "project_id": Uuid::new_v4(),
                "doc_key": "ARCH-1",
                "doc_type": "architecture",
                "markdown": "# 架构约束",
                "change_summary": null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_json(response).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("PM_MCP_DOCUMENT_WRITE"),
        "message must name the write permission: {body}"
    );
}