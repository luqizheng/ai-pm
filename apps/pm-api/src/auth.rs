//! Bearer authentication for pm-api (Slice B 登录链路 + T-021a 主体身份).
//!
//! Trust model (per `docs/superpowers/specs/2026-10-30-lmms-integration-design.md`
//! Phase 1): tokens are RS256 JWTs issued by the lmms token-service; pm-api
//! verifies signatures locally against the token-service JWKS via
//! `digua_common_jwt::JWKSCache` — no per-request round-trip.
//!
//! Three layers, outermost first (`docs/ai-pm-principal-identity-v1.md` §6):
//!
//! 1. [`require_bearer`] — verify the signature and the mandatory `tid` claim,
//!    then turn the raw claims into a typed [`AuthenticatedIdentity`].
//! 2. [`resolve_principal`] — reject service / cross-tenant tokens, map the
//!    `(provider, claims_tenant_key, subject)` triple to a `core.principal`
//!    (provisioning unknown identities as **inactive**), and insert
//!    [`AuthedPrincipal`].
//! 3. [`require_active`] — fail closed (`IAM_PERM_3002`) unless the principal
//!    is activated; `/health` and `GET /v1/pm/me` stay reachable for
//!    self-inspection.
//!
//! `AI_PM_AUTH_MODE=off` no longer removes the gates: it injects a fixed
//! `dev-off` human principal (`is_active=true`) so handlers still receive an
//! authenticated principal and audit rows still have an actor.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use digua_common_jwt::{JWKSCache, verify_token};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;

/// The `off`-mode principal (fixed id so tests and dev seeds are stable).
pub const DEV_OFF_PRINCIPAL: Uuid = Uuid::from_u128(0x00ff);

/// Environment-driven knobs for identity resolution.
#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    /// `AI_PM_FIRST_ADMIN_TENANT` — must pair with the subject below.
    pub first_admin_tenant: Option<String>,
    /// `AI_PM_FIRST_ADMIN_SUBJECT`.
    pub first_admin_subject: Option<String>,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        Self {
            first_admin_tenant: std::env::var("AI_PM_FIRST_ADMIN_TENANT")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            first_admin_subject: std::env::var("AI_PM_FIRST_ADMIN_SUBJECT")
                .ok()
                .filter(|v| !v.trim().is_empty()),
        }
    }

    /// 两者都配置才生效：只配一个不算首管理员，避免半配置下误授权。
    fn is_first_admin(&self, tenant_key: &str, subject: &str) -> bool {
        matches!(
            (&self.first_admin_tenant, &self.first_admin_subject),
            (Some(tenant), Some(sub)) if tenant == tenant_key && sub == subject
        )
    }
}

/// State captured by the auth middleware layers.
#[derive(Clone)]
pub struct AuthState {
    pub jwks: Option<Arc<JWKSCache>>,
    pub pool: PgPool,
    pub config: AuthConfig,
}

/// Typed claims: what the token says, before it is mapped to a principal.
#[derive(Debug, Clone)]
pub struct AuthenticatedIdentity {
    pub provider: &'static str,
    pub claims_tenant_key: String,
    pub subject: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub aud: Option<String>,
    pub client_id: Option<String>,
    pub ct: bool,
    pub username: Option<String>,
    pub nickname: Option<String>,
}

impl AuthenticatedIdentity {
    fn parse(claims: &Value) -> Option<Self> {
        Some(Self {
            provider: "lmms",
            claims_tenant_key: claim_text(claims.get("tid")?)?,
            subject: claim_text(claims.get("sub")?)?,
            roles: claim_list(claims.get("roles")),
            permissions: claim_list(claims.get("permissions")),
            aud: claim_text_opt(claims.get("aud")),
            client_id: claim_text_opt(claims.get("client_id")),
            ct: claims.get("ct").and_then(Value::as_bool).unwrap_or(false),
            username: claim_text_opt(claims.get("username")),
            nickname: claim_text_opt(claims.get("nickname")),
        })
    }

    fn is_service(&self) -> bool {
        self.roles.iter().any(|role| role == "service")
    }

    /// `aud` 存在 = 浏览器登录；无 `aud` 但有 `client_id` = API Key；
    /// 两者都没有（服务间/异常 token）= unknown。
    fn auth_mode(&self) -> &'static str {
        if self.aud.is_some() {
            "interactive"
        } else if self.client_id.is_some() {
            "api_key"
        } else {
            "unknown"
        }
    }

    pub fn has_permission(&self, code: &str) -> bool {
        self.permissions.iter().any(|granted| granted == code)
    }

    fn display_name(&self) -> &str {
        self.nickname
            .as_deref()
            .or(self.username.as_deref())
            .unwrap_or(&self.subject)
    }
}

/// The resolved caller: business principal + the identity row behind it.
#[derive(Debug, Clone)]
pub struct AuthedPrincipal {
    pub principal_id: Uuid,
    pub identity_id: Option<Uuid>,
    pub display_name: String,
    pub is_active: bool,
    pub is_admin: bool,
    pub auth_mode: String,
    pub provider: String,
    pub claims_tenant_key: String,
    pub subject: String,
    pub client: Option<String>,
}

impl AuthedPrincipal {
    pub fn audit_actor(&self) -> AuditActor {
        AuditActor {
            principal_id: self.principal_id,
            identity_id: self.identity_id,
            client: self.client.clone(),
            auth_mode: Some(self.auth_mode.clone()),
        }
    }
}

/// The bearer gate: verify the JWT, require `tid`, and forward typed claims.
pub async fn require_bearer(
    State(auth): State<AuthState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let Some(jwks) = auth.jwks.as_ref() else {
        return unauthorized("IAM_AUTH_1004", "bearer layer is not configured");
    };
    let Some(token) = request_token(&headers, request.uri()) else {
        return unauthorized("IAM_AUTH_1005", "missing bearer token");
    };
    match verify_token(token, jwks, None).await {
        Ok(claims) if claims.get("tid").is_some() => match AuthenticatedIdentity::parse(&claims) {
            Some(identity) => {
                let mut request = request;
                request.extensions_mut().insert(identity);
                next.run(request).await
            }
            // `sub` 缺失：签了名但没有主体，按无效 token 处理（fail closed）。
            None => unauthorized("IAM_AUTH_1004", "token missing subject claim (sub)"),
        },
        Ok(_) => unauthorized("IAM_AUTH_1004", "token missing tenant claim (tid)"),
        Err(_) => unauthorized("IAM_AUTH_1004", "invalid token"),
    }
}

/// Identity → `core.principal` mapping. Unknown triples are provisioned as
/// **inactive** (`is_active=false`) unless they match the configured first admin
/// pair; no project membership is granted here (`v1` §5 方案 C).
pub async fn resolve_principal(
    State(auth): State<AuthState>,
    request: Request,
    next: Next,
) -> Response {
    let identity = request.extensions().get::<AuthenticatedIdentity>().cloned();
    let Some(identity) = identity else {
        // Unreachable when the bearer layer is present; fail closed anyway.
        return unauthorized("IAM_AUTH_1005", "missing bearer token");
    };
    if identity.is_service() {
        return forbidden("IAM_PERM_3003", "service tokens are not accepted on business endpoints");
    }
    if identity.ct {
        return forbidden("IAM_PERM_3004", "cross-tenant tokens are not accepted");
    }
    match resolve_or_provision(&auth, &identity).await {
        Ok(principal) => {
            let mut request = request;
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        Err(err) => {
            tracing::error!(error = %err, "identity resolution failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "code": "IAM_AUTH_1006",
                    "message": "identity resolution failed"
                })),
            )
                .into_response()
        }
    }
}

/// Fail closed on inactive principals. Self-inspection stays available so a
/// caller can learn its own activation state (`v1` §6 鉴权矩阵).
pub async fn require_active(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    if is_self_service_path(&path) || request.method() == axum::http::Method::OPTIONS {
        return next.run(request).await;
    }
    match request.extensions().get::<AuthedPrincipal>() {
        Some(principal) if principal.is_active => next.run(request).await,
        Some(_) => forbidden("IAM_PERM_3002", "principal is not activated"),
        // Unreachable when `resolve_principal` is present; fail closed anyway.
        None => unauthorized("IAM_AUTH_1005", "missing bearer token"),
    }
}

fn is_self_service_path(path: &str) -> bool {
    path == "/health" || (path == "/v1/pm/me")
}

/// `AI_PM_AUTH_MODE=off`: inject the fixed `dev-off` principal instead of
/// removing the gates. The row is materialised only for write requests so an
/// empty test database keeps answering zero totals for `GET /v1/pm/principals`.
pub async fn inject_dev_principal(
    State(auth): State<AuthState>,
    request: Request,
    next: Next,
) -> Response {
    let writes = !matches!(
        request.method(),
        &axum::http::Method::GET
            | &axum::http::Method::HEAD
            | &axum::http::Method::OPTIONS
    );
    if writes {
        if let Err(err) = sqlx::query(
            "INSERT INTO core.principal (id, kind, display_name, is_active) \
             VALUES ($1, 'human', 'dev-off', true) ON CONFLICT (id) DO NOTHING",
        )
        .bind(DEV_OFF_PRINCIPAL)
        .execute(&auth.pool)
        .await
        {
            tracing::error!(error = %err, "failed to materialise the dev-off principal");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "code": "IAM_AUTH_1006", "message": "identity resolution failed" })),
            )
                .into_response();
        }
    }
    let mut request = request;
    request.extensions_mut().insert(AuthedPrincipal {
        principal_id: DEV_OFF_PRINCIPAL,
        identity_id: None,
        display_name: "dev-off".to_string(),
        is_active: true,
        is_admin: true,
        auth_mode: "off".to_string(),
        provider: "dev-off".to_string(),
        claims_tenant_key: "-".to_string(),
        subject: "dev-off".to_string(),
        client: None,
    });
    next.run(request).await
}

async fn resolve_or_provision(
    auth: &AuthState,
    identity: &AuthenticatedIdentity,
) -> Result<AuthedPrincipal, sqlx::Error> {
    let existing: Option<(Uuid, Uuid, bool, String)> = sqlx::query_as(
        "SELECT pi.id, pi.principal_id, p.is_active, p.display_name \
           FROM core.principal_identity pi \
           JOIN core.principal p ON p.id = pi.principal_id \
          WHERE pi.provider = $1 AND pi.claims_tenant_key = $2 AND pi.subject = $3",
    )
    .bind(identity.provider)
    .bind(&identity.claims_tenant_key)
    .bind(&identity.subject)
    .fetch_optional(&auth.pool)
    .await?;

    let (identity_id, principal_id, is_active, display_name) = match existing {
        Some(row) => {
            // 节流：5 分钟内的重复请求不再写库。
            sqlx::query(
                "UPDATE core.principal_identity SET last_seen_at = now() \
                  WHERE id = $1 AND last_seen_at < now() - interval '5 minutes'",
            )
            .bind(row.0)
            .execute(&auth.pool)
            .await?;
            row
        }
        None => provision_principal(auth, identity).await?,
    };

    Ok(AuthedPrincipal {
        principal_id,
        identity_id: Some(identity_id),
        display_name,
        is_active,
        // 临时口径（v1 §6.2）：T-021b 起改读 core.tenant_member.role。
        is_admin: identity.roles.iter().any(|role| role == "tenant_admin"),
        auth_mode: identity.auth_mode().to_string(),
        provider: identity.provider.to_string(),
        claims_tenant_key: identity.claims_tenant_key.clone(),
        subject: identity.subject.clone(),
        client: identity.client_id.clone(),
    })
}

/// 首次见到未知三元组：建 principal（默认未激活）+ identity 行。
/// 并发输家（identity 行已存在）删掉自己刚建的 principal 后复用赢家的行。
async fn provision_principal(
    auth: &AuthState,
    identity: &AuthenticatedIdentity,
) -> Result<(Uuid, Uuid, bool, String), sqlx::Error> {
    let is_active = auth
        .config
        .is_first_admin(&identity.claims_tenant_key, &identity.subject);
    let display_name = identity.display_name().to_string();
    let mut tx = auth.pool.begin().await?;

    let principal_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', $2, $3)",
    )
    .bind(principal_id)
    .bind(&display_name)
    .bind(is_active)
    .execute(&mut *tx)
    .await?;

    let inserted: Option<(Uuid, Uuid)> = sqlx::query_as(
        "INSERT INTO core.principal_identity \
             (provider, claims_tenant_key, subject, principal_id) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (provider, claims_tenant_key, subject) DO NOTHING \
         RETURNING id, principal_id",
    )
    .bind(identity.provider)
    .bind(&identity.claims_tenant_key)
    .bind(&identity.subject)
    .bind(principal_id)
    .fetch_optional(&mut *tx)
    .await?;

    let resolved = match inserted {
        Some((identity_id, winner)) => (identity_id, winner, is_active, display_name),
        None => {
            // 并发输家：刚建的 principal 未被任何行引用，直接删掉。
            sqlx::query("DELETE FROM core.principal WHERE id = $1")
                .bind(principal_id)
                .execute(&mut *tx)
                .await?;
            let (identity_id, winner, winner_active, winner_name): (Uuid, Uuid, bool, String) =
                sqlx::query_as(
                    "SELECT pi.id, pi.principal_id, p.is_active, p.display_name \
                       FROM core.principal_identity pi \
                       JOIN core.principal p ON p.id = pi.principal_id \
                      WHERE pi.provider = $1 AND pi.claims_tenant_key = $2 AND pi.subject = $3",
                )
                .bind(identity.provider)
                .bind(&identity.claims_tenant_key)
                .bind(&identity.subject)
                .fetch_one(&mut *tx)
                .await?;
            (identity_id, winner, winner_active, winner_name)
        }
    };
    tx.commit().await?;
    Ok(resolved)
}

/// SSE 路径：`EventSource` 不能带自定义头，只能把 token 放 query。
const SSE_PATH: &str = "/v1/pm/events/stream";

/// MCP endpoint gate (integration spec Phase 1 §3): every `/mcp` call needs
/// `PM_MCP_READ`; a `tools/call` for a write tool additionally needs its
/// per-tool permission. Only POSTs are parsed — GET is the SSE transport.
const MCP_READ_PERMISSION: &str = "PM_MCP_READ";
const MCP_BODY_LIMIT: usize = 256 * 1024;

/// tool name → additionally required permission (read tools need only
/// `PM_MCP_READ`). `PM_MCP_TASK_UPDATE` stays unassigned until a task-update
/// tool exists.
const MCP_TOOL_PERMISSIONS: [(&str, &str); 5] = [
    ("pm_submit_technical_design", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_submit_document", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_propose_document", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_confirm_document_proposal", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_veto_document_revision", "PM_MCP_DOCUMENT_WRITE"),
];

/// Permission gate layered *inside* [`require_bearer`], so claims are already
/// verified and present in the request extensions.
pub async fn require_mcp_permission(
    identity: Option<Extension<AuthenticatedIdentity>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(Extension(identity)) = identity else {
        // Unreachable when the bearer layer is present; fail closed anyway.
        return unauthorized("IAM_AUTH_1005", "missing bearer token");
    };
    if !identity.has_permission(MCP_READ_PERMISSION) {
        return forbidden("IAM_PERM_3001", "permission denied: PM_MCP_READ required");
    }
    if request.method() != axum::http::Method::POST {
        return next.run(request).await;
    }

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, MCP_BODY_LIMIT).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({
                    "code": "IAM_REQUEST_TOO_LARGE",
                    "message": "mcp request body exceeds the 256 KiB limit"
                })),
            )
                .into_response()
        }
    };
    if let Some(permission) = required_tool_permission(&bytes) {
        if !identity.has_permission(permission) {
            return forbidden(
                "IAM_PERM_3001",
                &format!("permission denied: {permission} required"),
            );
        }
    }
    next.run(Request::from_parts(parts, axum::body::Body::from(bytes)))
        .await
}

/// Maps a `tools/call` body to the write permission it needs; read tools and
/// non-tool JSON-RPC frames need nothing beyond `PM_MCP_READ`.
fn required_tool_permission(body: &[u8]) -> Option<&'static str> {
    let value: Value = serde_json::from_slice(body).ok()?;
    if value.get("method")?.as_str()? != "tools/call" {
        return None;
    }
    let tool = value.get("params")?.get("name")?.as_str()?;
    MCP_TOOL_PERMISSIONS
        .iter()
        .find(|(name, _)| *name == tool)
        .map(|(_, permission)| *permission)
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

/// `tid` 在上游是数值，`sub` 是字符串；统一按文本读，数值型转字符串。
fn claim_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|n| n.to_string()))
        .or_else(|| value.as_u64().map(|n| n.to_string()))
}

fn claim_text_opt(value: Option<&Value>) -> Option<String> {
    match value {
        Some(value) => claim_text(value),
        None => None,
    }
}

fn claim_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn unauthorized(code: &str, message: &str) -> Response {
    let body = json!({ "code": code, "message": message });
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}
fn forbidden(code: &str, message: &str) -> Response {
    let body = json!({ "code": code, "message": message });
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}
