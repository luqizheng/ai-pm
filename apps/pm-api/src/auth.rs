//! Bearer authentication for pm-api (Slice B 登录链路).
//!
//! Trust model (per `docs/superpowers/specs/2026-10-30-lmms-integration-design.md`
//! Phase 1): tokens are RS256 JWTs issued by the lmms token-service; pm-api
//! verifies signatures locally against the token-service JWKS via
//! `digua_common_jwt::JWKSCache` — no per-request round-trip.
//!
//! Two claims rules, mirroring the lmms api-gateway (`verify_token(..., None)`
//! plus a mandatory tenant claim): the audience is NOT checked here so both
//! browser-login tokens (`aud="coder-micro-services"`) and machine tokens
//! (client_credentials, no `aud`) are accepted, and the `tid` claim is
//! mandatory. Verification failures are fail-closed 401s; the response body
//! uses the lmms error-code convention (`IAM_AUTH_1004/1005`) so the frontend
//! `@digua/http` client can translate them. The AppError conversion layer
//! belongs to spec Phase 2 and is deliberately not introduced here.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use digua_common_jwt::{JWKSCache, verify_token};
use serde_json::{Value, json};

/// State captured by the auth middleware layer.
#[derive(Clone)]
pub struct AuthState {
    pub jwks: Arc<JWKSCache>,
}

/// The bearer gate: verify the JWT, require `tid`, then forward the request
/// with the verified claims inserted into the extensions (for handlers and
/// future per-permission checks to consume).
pub async fn require_bearer(
    State(auth): State<AuthState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let Some(token) = request_token(&headers, request.uri()) else {
        return unauthorized("IAM_AUTH_1005", "missing bearer token");
    };
    match verify_token(token, &auth.jwks, None).await {
        Ok(claims) if claims.get("tid").is_some() => {
            let mut request = request;
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Ok(_) => unauthorized("IAM_AUTH_1004", "token missing tenant claim (tid)"),
        Err(_) => unauthorized("IAM_AUTH_1004", "invalid token"),
    }
}

/// SSE 路径：`EventSource` 不能带自定义头，只能把 token 放 query。
const SSE_PATH: &str = "/v1/pm/events/stream";

/// MCP endpoint gate (integration spec Phase 1 §3): every `/mcp` call needs
/// `PM_MCP_READ`. Per-tool write permissions (`PM_MCP_DOCUMENT_WRITE` /
/// `PM_MCP_TASK_UPDATE`) are a follow-up — the JSON-RPC body is not parsed here.
const MCP_READ_PERMISSION: &str = "PM_MCP_READ";

/// Permission gate layered *inside* [`require_bearer`], so claims are already
/// verified and present in the request extensions.
pub async fn require_mcp_read(
    claims: Option<Extension<Value>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(Extension(claims)) = claims else {
        // Unreachable when the bearer layer is present; fail closed anyway.
        return unauthorized("IAM_AUTH_1005", "missing bearer token");
    };
    if has_permission(&claims, MCP_READ_PERMISSION) {
        next.run(request).await
    } else {
        forbidden("IAM_PERM_3001", "permission denied: PM_MCP_READ required")
    }
}

/// `permissions` is the array claim token-service issues (`["IAM_SYSTEM_READ"]`).
fn has_permission(claims: &Value, code: &str) -> bool {
    claims
        .get("permissions")
        .and_then(Value::as_array)
        .is_some_and(|granted| granted.iter().any(|item| item.as_str() == Some(code)))
}

/// Header first; the query fallback is scoped to the one SSE endpoint so the
/// token-in-URL surface never widens beyond it.
fn request_token<'a>(headers: &'a HeaderMap, uri: &'a axum::http::Uri) -> Option<&'a str> {
    bearer_token(headers).or_else(|| {
        if uri.path() == SSE_PATH {
            query_access_token(uri)
        } else {
            None
        }
    })
}

/// Extracts `access_token` from the raw query string (JWTs are base64url + dots,
/// no percent-decoding needed).
fn query_access_token(uri: &axum::http::Uri) -> Option<&str> {
    uri.query()?.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == "access_token" && !value.is_empty()).then_some(value)
    })
}

/// Extracts the raw JWT from an `Authorization: Bearer <jwt>` header.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .filter(|token| !token.trim().is_empty())
        .map(str::trim)
}

fn unauthorized(code: &str, message: &str) -> Response {
    let body = json!({ "code": code, "message": message });
    (StatusCode::UNAUTHORIZED, Json(Value::from(body))).into_response()
}
fn forbidden(code: &str, message: &str) -> Response {
    let body = json!({ "code": code, "message": message });
    (StatusCode::FORBIDDEN, Json(Value::from(body))).into_response()
}
