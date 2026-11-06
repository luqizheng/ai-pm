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
//! 2. [`resolve_principal`] — reject service / cross-tenant tokens, mirror the
//!    upstream `tid` into `core.tenant`, map `(provider, tenant_id, subject)`
//!    to a `core.principal` (provisioning unknown identities as **inactive**
//!    tenant `member`s), and insert [`AuthedPrincipal`].
//! 3. [`require_active`] — fail closed (`IAM_PERM_3002`) unless the principal
//!    is activated; `/health` and `GET /v1/pm/me` stay reachable for
//!    self-inspection.
//!
//! 租户角色（`is_admin`）每请求查 `core.tenant_member.role`，token `roles`
//! 不参与授权（ADR-0001 D5）——降级/删除成员后旧 JWT 立即失去管理权。
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
use pm_domain::TenantRole;

/// The `off`-mode principal (fixed id so tests and dev seeds are stable).
pub const DEV_OFF_PRINCIPAL: Uuid = Uuid::from_u128(0x00ff);

/// 系统租户：由 `0019_tenant.sql` 幂等种子，`dev-off` 与测试夹具共用（Q6）。
pub const SYSTEM_TENANT: Uuid = Uuid::from_u128(0x10);

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
    /// lmms token-service v1.7.0 起签发的用途声明（签发通道权威值：`user_login` /
    /// `api_key` / `auth_code` / `service` / `resource`）；客户端不可自报。存量旧令牌
    /// 无该键，`auth_mode()` 回落旧推断。契约：ai-pm `docs/superpowers/specs` 转录自
    /// lmms `2026-11-04-token-purpose-claim-design.md` §4.7。
    pub purpose: Option<String>,
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
            purpose: claim_text_opt(claims.get("purpose")),
            ct: claims.get("ct").and_then(Value::as_bool).unwrap_or(false),
            username: claim_text_opt(claims.get("username")),
            nickname: claim_text_opt(claims.get("nickname")),
        })
    }

    fn is_service(&self) -> bool {
        self.roles.iter().any(|role| role == "service")
    }

    /// 令牌类别判定。lmms v1.7.0 起 `purpose` 为签发方权威声明，存在即取代
    /// `aud`/`client_id` 推断（词表映射：user_login→interactive，
    /// api_key/auth_code→api_key，service/resource→service）；未知枚举值按
    /// unknown（fail-closed，消费方不得放行）。缺失=存量旧令牌，沿用旧推断：
    /// `aud` 存在 = 浏览器登录；无 `aud` 但有 `client_id` = API Key；
    /// 两者都没有（服务间/异常 token）= unknown。
    fn auth_mode(&self) -> &'static str {
        match self.purpose.as_deref() {
            Some("user_login") => "interactive",
            Some("api_key") | Some("auth_code") => "api_key",
            Some("service") | Some("resource") => "service",
            Some(_) => "unknown",
            None => {
                if self.aud.is_some() {
                    "interactive"
                } else if self.client_id.is_some() {
                    "api_key"
                } else {
                    "unknown"
                }
            }
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
    /// 租户维度（T-021b）：所有租户业务写入的 `tenant_id` 来源。
    pub tenant_id: Uuid,
    /// `core.tenant_member.role` 的当前值；无成员行时落 `Member`（最小权限）。
    pub tenant_role: TenantRole,
    /// 是否可进入租户管理面 = `tenant_role.is_tenant_admin()`。
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
            claimed_device_id: None,
        }
    }

    /// T-088：请求级真人身份（`HumanActor`）——**唯一生产点**在鉴权层，供 `/mcp`
    /// 工具层经 rmcp `RequestContext.extensions` 只读取用。
    pub fn human_actor(&self) -> pm_application::caller::HumanActor {
        pm_application::caller::HumanActor {
            principal_id: self.principal_id,
            tenant_id: self.tenant_id,
            identity_id: self.identity_id,
            client: self.client.clone(),
            auth_mode: self.auth_mode.clone(),
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
    // 公开路径（健康探针 / 接入引导）不鉴权：agent 在拿到 key 之前就要拉取
    // 接入指引，走不到这一步就没有「粘贴 URL → 帮我安装」。
    if is_public_path(request.uri().path()) {
        return next.run(request).await;
    }
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
        Err(e) => {
            // 2026-11-05 排障：Err 此前被吞成通用 "invalid token"，本地验签
            // VALID 却 401 时无从定位；打出具体变体（签名/过期/未知 kid/格式）。
            tracing::warn!(error = %e, "bearer token verification failed");
            unauthorized("IAM_AUTH_1004", "invalid token")
        }
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
    if is_public_path(request.uri().path()) {
        return next.run(request).await;
    }
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
            // T-088：HumanActor 与 AuthedPrincipal 同点生产，供 /mcp 工具层只读。
            request.extensions_mut().insert(principal.human_actor());
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
    if is_public_path(&path) || is_self_service_path(&path) || request.method() == axum::http::Method::OPTIONS {
        return next.run(request).await;
    }
    match request.extensions().get::<AuthedPrincipal>() {
        Some(principal) if principal.is_active => next.run(request).await,
        Some(_) => forbidden("IAM_PERM_3002", "principal is not activated"),
        // Unreachable when `resolve_principal` is present; fail closed anyway.
        None => unauthorized("IAM_AUTH_1005", "missing bearer token"),
    }
}

/// 公开路径（免鉴权）：健康探针与执行端接入引导。
///
/// - `/health`：部署就绪探针（runbook §二 的部署校验）。
/// - `/install`、`/install/{tool}`：执行端接入指引——agent 在拿到 API Key
///   **之前**就要拉取它，所以必须免鉴权。
///
/// 公开路径不返回任何凭据：指引里只有配置片段与 `<API_KEY>` 占位符。
pub fn is_public_path(path: &str) -> bool {
    path == "/health" || path == "/install" || path.starts_with("/install/")
}

fn is_self_service_path(path: &str) -> bool {
    // `/v1/pm/me`：未激活主体唯一的自查入口（v1 §6 鉴权矩阵）。
    // Team Onboarding（0044）：建团认领与邀请接受/预览也是未激活 principal 的
    // 合法动作（产品裁决 Q1：首登必须建团或入团）；全部精确匹配，仍在
    // bearer 层之内（拒绝匿名），不扩大公开面。
    path == "/health"
        || path == "/v1/pm/me"
        || path == "/v1/pm/teams"
        || path == "/v1/pm/invitations/preview"
        || path == "/v1/pm/invitations/accept"
        || path == "/v1/pm/invitation-links/preview"
        || path == "/v1/pm/invitation-links/accept"
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
        // principal 与 tenant_member 一起物化到系统租户（Q6）；只随写请求落库，
        // 保证空库下 `GET` 仍返回零总数。
        let seed = async {
            sqlx::query(
                "INSERT INTO core.principal (id, kind, display_name, is_active) \
                 VALUES ($1, 'human', 'dev-off', true) ON CONFLICT (id) DO NOTHING",
            )
            .bind(DEV_OFF_PRINCIPAL)
            .execute(&auth.pool)
            .await?;
            sqlx::query(
                "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
                 VALUES ($1, $2, 'owner') ON CONFLICT (tenant_id, principal_id) DO NOTHING",
            )
            .bind(SYSTEM_TENANT)
            .bind(DEV_OFF_PRINCIPAL)
            .execute(&auth.pool)
            .await?;
            Ok::<(), sqlx::Error>(())
        }
        .await;
        if let Err(err) = seed {
            tracing::error!(error = %err, "failed to materialise the dev-off principal");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "code": "IAM_AUTH_1006", "message": "identity resolution failed" })),
            )
                .into_response();
        }
    }
    let mut request = request;
    let principal = AuthedPrincipal {
        principal_id: DEV_OFF_PRINCIPAL,
        identity_id: None,
        display_name: "dev-off".to_string(),
        is_active: true,
        tenant_id: SYSTEM_TENANT,
        tenant_role: TenantRole::Owner,
        is_admin: true,
        auth_mode: "off".to_string(),
        provider: "dev-off".to_string(),
        claims_tenant_key: "-".to_string(),
        subject: "dev-off".to_string(),
        client: None,
    };
    request.extensions_mut().insert(principal.human_actor());
    request.extensions_mut().insert(principal);
    next.run(request).await
}

async fn resolve_or_provision(
    auth: &AuthState,
    identity: &AuthenticatedIdentity,
) -> Result<AuthedPrincipal, sqlx::Error> {
    // Q1：上游 tid 1:1 镜像为 core.tenant（自动建即生效）。
    let tenant_id = ensure_tenant(auth, identity).await?;

    let existing: Option<(Uuid, Uuid, bool, String)> = sqlx::query_as(
        "SELECT pi.id, pi.principal_id, p.is_active, p.display_name \
           FROM core.principal_identity pi \
           JOIN core.principal p ON p.id = pi.principal_id \
          WHERE pi.provider = $1 AND pi.tenant_id = $2 AND pi.subject = $3",
    )
    .bind(identity.provider)
    .bind(tenant_id)
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
        None => provision_principal(auth, identity, tenant_id).await?,
    };

    // D5：角色唯一事实源是库内的 tenant_member.role，不是 token roles。
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member \
          WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_optional(&auth.pool)
    .await?;
    let tenant_role = role
        .as_deref()
        .and_then(TenantRole::parse)
        .unwrap_or_else(TenantRole::min_privilege);

    Ok(AuthedPrincipal {
        principal_id,
        identity_id: Some(identity_id),
        display_name,
        is_active,
        tenant_id,
        tenant_role,
        is_admin: tenant_role.is_tenant_admin(),
        auth_mode: identity.auth_mode().to_string(),
        provider: identity.provider.to_string(),
        claims_tenant_key: identity.claims_tenant_key.clone(),
        subject: identity.subject.clone(),
        client: identity.client_id.clone(),
    })
}

/// 按 `(provider, tid)` 找租户，没有就建（显示名取 `provider:tid`）。
/// 并发输家（`ON CONFLICT` 未插入）再读一次赢家的行。
async fn ensure_tenant(
    auth: &AuthState,
    identity: &AuthenticatedIdentity,
) -> Result<Uuid, sqlx::Error> {
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM core.tenant WHERE provider = $1 AND external_key = $2",
    )
    .bind(identity.provider)
    .bind(&identity.claims_tenant_key)
    .fetch_optional(&auth.pool)
    .await?;
    if let Some(id) = existing {
        return Ok(id);
    }

    let display_name = format!("{}:{}", identity.provider, identity.claims_tenant_key);
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (provider, external_key) DO NOTHING \
         RETURNING id",
    )
    .bind(identity.provider)
    .bind(&identity.claims_tenant_key)
    .bind(&display_name)
    .fetch_optional(&auth.pool)
    .await?;

    match inserted {
        Some(id) => Ok(id),
        None => Ok(sqlx::query_scalar(
            "SELECT id FROM core.tenant WHERE provider = $1 AND external_key = $2",
        )
        .bind(identity.provider)
        .bind(&identity.claims_tenant_key)
        .fetch_one(&auth.pool)
        .await?),
    }
}

/// 首次见到未知三元组：建 principal（默认未激活）+ identity 行 + tenant_member 行。
/// 并发输家（identity 行已存在）删掉自己刚建的 principal 后复用赢家的行。
///
/// 首次管理员的 env 对（`AI_PM_FIRST_ADMIN_TENANT` + `SUBJECT`）命中时：
/// principal 直接激活，且 `tenant_member.role = 'owner'`（Q2 冷启动唯一入口）。
async fn provision_principal(
    auth: &AuthState,
    identity: &AuthenticatedIdentity,
    tenant_id: Uuid,
) -> Result<(Uuid, Uuid, bool, String), sqlx::Error> {
    let is_first_admin = auth
        .config
        .is_first_admin(&identity.claims_tenant_key, &identity.subject);
    let is_active = is_first_admin;
    let role = if is_first_admin { "owner" } else { "member" };
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
             (provider, tenant_id, subject, principal_id) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (provider, tenant_id, subject) DO NOTHING \
         RETURNING id, principal_id",
    )
    .bind(identity.provider)
    .bind(tenant_id)
    .bind(&identity.subject)
    .bind(principal_id)
    .fetch_optional(&mut *tx)
    .await?;

    let resolved = match inserted {
        Some((identity_id, winner)) => {
            // 成员行必须在 identity 抢占成功之后插入：并发输家会删 principal，
            // 若提前插了 tenant_member 会因外键而删不掉。
            sqlx::query(
                "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
                 VALUES ($1, $2, $3) ON CONFLICT (tenant_id, principal_id) DO NOTHING",
            )
            .bind(tenant_id)
            .bind(winner)
            .bind(role)
            .execute(&mut *tx)
            .await?;
            (identity_id, winner, is_active, display_name)
        }
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
                      WHERE pi.provider = $1 AND pi.tenant_id = $2 AND pi.subject = $3",
                )
                .bind(identity.provider)
                .bind(tenant_id)
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
///
/// ACP 通信面（设计稿 docs/ui/s03-acp/smark-pm-s03-acp-slice-design-
/// acp01-02-08-v1.md V1.1 §1.6，R-ACP-8 已裁）：新增工具不再挂
/// `PM_MCP_READ` 语义——`PM_MCP_AGENT_REPORT` = 执行端运行事件上报（Agent
/// 侧事实，永无人类确认语义）；`PM_MCP_COMMAND` = 命令通道收发。人类正式
/// 确认永不入任何 MCP 权限码（A*：仅可信 REST/UI）。字典落地随 V-05-1
/// 迁移批（Code Registry 面目前沿用 JWT permissions claim 直比）。
const MCP_TOOL_PERMISSIONS: [(&str, &str); 9] = [
    ("pm_submit_technical_design", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_submit_document", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_propose_document", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_confirm_document_proposal", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_veto_document_revision", "PM_MCP_DOCUMENT_WRITE"),
    ("pm_report_interaction", "PM_MCP_AGENT_REPORT"),
    ("pm_report_consumption", "PM_MCP_AGENT_REPORT"),
    ("pm_poll_commands", "PM_MCP_COMMAND"),
    ("pm_ack_command", "PM_MCP_COMMAND"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(claims: Value) -> AuthenticatedIdentity {
        AuthenticatedIdentity::parse(&claims).expect("test claims must parse")
    }

    fn base_claims() -> Value {
        json!({
            "tid": 1,
            "sub": "usr-1",
            "roles": ["member"],
            "permissions": ["pm.requirement.read"],
        })
    }

    #[test]
    fn purpose_user_login_maps_to_interactive() {
        let mut c = base_claims();
        c["purpose"] = json!("user_login");
        c["aud"] = json!("coder-micro-services");
        assert_eq!(identity(c).auth_mode(), "interactive");
    }

    #[test]
    fn purpose_api_key_and_auth_code_map_to_api_key() {
        for p in ["api_key", "auth_code"] {
            let mut c = base_claims();
            c["purpose"] = json!(p);
            c["client_id"] = json!("key-1");
            assert_eq!(identity(c).auth_mode(), "api_key", "purpose={p}");
        }
    }

    #[test]
    fn purpose_is_authoritative_over_aud_inference() {
        // OAuth/机器令牌不该带 aud；若带，purpose 仍赢——权威声明优先于推断。
        let mut c = base_claims();
        c["purpose"] = json!("api_key");
        c["aud"] = json!("coder-micro-services");
        assert_eq!(identity(c).auth_mode(), "api_key");
    }

    #[test]
    fn purpose_service_and_resource_map_to_service() {
        for p in ["service", "resource"] {
            let mut c = base_claims();
            c["purpose"] = json!(p);
            assert_eq!(identity(c).auth_mode(), "service", "purpose={p}");
        }
    }

    #[test]
    fn unknown_purpose_fails_closed_to_unknown() {
        let mut c = base_claims();
        c["purpose"] = json!("interactive"); // 伪造的枚举外值
        c["aud"] = json!("coder-micro-services");
        assert_eq!(identity(c).auth_mode(), "unknown");
    }

    #[test]
    fn legacy_token_without_purpose_keeps_old_inference() {
        // 存量旧令牌：无 purpose，沿用 aud/client_id 推断（兼容语义，契约 §4.7-3）。
        let mut c = base_claims();
        c["aud"] = json!("coder-micro-services");
        assert_eq!(identity(c).auth_mode(), "interactive");

        let mut c = base_claims();
        c["client_id"] = json!("key-1");
        assert_eq!(identity(c).auth_mode(), "api_key");

        assert_eq!(identity(base_claims()).auth_mode(), "unknown");
    }

    #[test]
    fn purpose_non_string_is_coerced_then_fails_closed() {
        // claim_text 对数字做串化（与 tid 同 helper）：42 → Some("42") →
        // 枚举外值 → unknown（fail-closed），绝不落回 legacy 推断。
        let mut c = base_claims();
        c["purpose"] = json!(42);
        c["aud"] = json!("coder-micro-services");
        let id = identity(c);
        assert_eq!(id.purpose.as_deref(), Some("42"));
        assert_eq!(id.auth_mode(), "unknown");
    }
}
