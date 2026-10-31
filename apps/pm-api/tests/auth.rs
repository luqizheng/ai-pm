//! Auth middleware tests (Slice B 登录链路): drive `router_with_auth` with
//! `tower::ServiceExt::oneshot` — no socket — and assert the bearer contract:
//! missing token → 401 `IAM_AUTH_1005`; forged/undecodable token → 401
//! `IAM_AUTH_1004`; a validly signed token carrying `tid` passes through to
//! the real route; a validly signed token without `tid` → 401; and the
//! `off` mode (`router`, no auth layer) keeps the legacy open behaviour.
//!
//! Signing uses `digua_common_jwt::sign_token` against the test-only RSA
//! keypair in `tests/keys/` (never used in production — the runtime trust
//! anchor is the token-service JWKS URL); verification uses
//! `JWKSCache::from_pem`, which pins the same `kid="default"` convention as
//! token-service.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{JWKSCache, sign_token};
use pm_api::routes::{router, router_with_auth};
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

/// A JWKSCache pinned to the test public key (kid="default"), mirroring what
/// a live token-service JWKS fetch would produce.
fn test_jwks() -> Arc<JWKSCache> {
    Arc::new(JWKSCache::from_pem(PUBLIC_PEM).expect("test public key must parse"))
}

/// Signs a token the way token-service does for user logins: RS256, `tid`
/// tenant claim, plus the login-claims payload. `tid` can be omitted to test
/// the tenant-claim gate.
fn sign(claims: Value, ttl: Duration) -> String {
    sign_token(PRIVATE_PEM, &claims, ttl).expect("test token must sign")
}

fn bearer(token: Option<&str>) -> Request<Body> {
    let project = uuid::Uuid::new_v4();
    let mut builder = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/projects/{project}/notifications"));
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).unwrap()
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn missing_token_is_unauthorized(pool: PgPool) {
    let app = router_with_auth(pool, test_jwks());
    let response = app.oneshot(bearer(None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["code"], json!("IAM_AUTH_1005"), "missing token code");
    assert!(body["message"].is_string());
}

#[sqlx::test(migrations = "../../migrations")]
async fn forged_token_is_unauthorized(pool: PgPool) {
    let app = router_with_auth(pool, test_jwks());
    // Not signed by the trusted key — the JWKS has one key (kid="default"),
    // so an arbitrary string must fail verification.
    let response = app
        .oneshot(bearer(Some("not-a-jwt")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["code"], json!("IAM_AUTH_1004"), "invalid token code");
}

#[sqlx::test(migrations = "../../migrations")]
async fn token_without_tenant_claim_is_unauthorized(pool: PgPool) {
    let app = router_with_auth(pool, test_jwks());
    // Correctly signed by the trusted key but no `tid` — the gateway treats
    // `tid` as mandatory for downstream calls, pm-api mirrors that.
    let token = sign(json!({ "sub": "usr-1", "username": "alice" }), Duration::from_secs(300));
    let response = app.oneshot(bearer(Some(&token))).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["code"], json!("IAM_AUTH_1004"), "missing tid code");
}

#[sqlx::test(migrations = "../../migrations")]
async fn valid_token_reaches_the_real_route(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "AUTH-1".to_string(),
            project_name: "auth project".to_string(),
        },
    )
    .await
    .unwrap();
    let app = router_with_auth(pool, test_jwks());
    let token = sign(
        json!({ "sub": boot.human_id, "tid": 1, "username": "alice", "roles": ["tenant_admin"], "permissions": [] }),
        Duration::from_secs(300),
    );
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/projects/{}/notifications", boot.project_id))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a valid bearer token must reach the notifications feed"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn off_mode_keeps_legacy_open_behaviour(pool: PgPool) {
    // `router` (what main.rs serves with AI_PM_AUTH_MODE=off) has no auth
    // layer: requests pass through exactly as before Slice B.
    let app = router(pool);
    let response = app.oneshot(bearer(None)).await.unwrap();
    assert_ne!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "off mode must not reject unauthenticated requests"
    );
}
