//! MCP endpoint permission gate (integration spec Phase 1 §3): every `/mcp`
//! call requires `PM_MCP_READ`; missing permission → 403 `IAM_PERM_3001`,
//! while the rest of the API is unaffected by the permission claim.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use digua_common_jwt::{JWKSCache, sign_token};
use pm_api::routes::router_with_auth;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

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

fn mcp_request(token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", "127.0.0.1:8090")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(INITIALIZE_BODY)).unwrap()
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn mcp_without_token_is_unauthorized(pool: PgPool) {
    let response = router_with_auth(pool, test_jwks())
        .oneshot(mcp_request(None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn mcp_without_mcp_read_is_forbidden(pool: PgPool) {
    let token = sign(&["IAM_SYSTEM_READ"]);
    let response = router_with_auth(pool, test_jwks())
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
    let response = router_with_auth(pool, test_jwks())
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
    let response = router_with_auth(pool, test_jwks())
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