//! Team Onboarding HTTP 面（设计稿 `docs/team-onboarding/`，产品裁决 2026-11-06）。
//!
//! 路由分两组：
//! - 租户管理组（`/v1/pm/tenants/{tenant_id}*`）：建团/档案/邀请/链接，owner/admin
//!   守卫随 `team_llm_guard`（跨租户 404 + 非管理员 403）。
//! - 自助组（`/v1/pm/teams`、`/v1/pm/invitations/*`、`/v1/pm/invitation-links/*`）：
//!   未激活 principal 也可调（`auth::is_self_service_path` 放行）——这正是
//!   onboarding 的入口（认领建团 / 接受邀请）。仍需 bearer。
//!
//! 错误契约：`{message}` 单字段（support.rs 口径）；邀请域的终态错误使用稳定
//! machine message（`invitation_expired` / `invitation_revoked` /
//! `invitation_already_accepted` / `invitation_link_exhausted` /
//! `email_mismatch` / `invitation_tenant_mismatch` / `team_suspended` /
//! `invitation_not_found` / `slug_already_taken` / `pending_invitation_exists` /
//! `team_already_exists`），前端据其渲染结果态（设计稿 Screen 07）。

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use pm_application::team_onboarding as svc;
use pm_application::team_onboarding::TeamOnboardingError;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::team_llm_guard::{err_not_found, guard_tenant};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        // 建团（认领当前 token 租户）：自助路径，未激活 principal 可调。
        .route("/v1/pm/teams", post(create_team_handler))
        // 团队档案：成员读 / admin 写。
        .route(
            "/v1/pm/tenants/{tenant_id}",
            get(team_profile_handler).patch(update_team_handler),
        )
        // 邮箱邀请管理（admin）。
        .route(
            "/v1/pm/tenants/{tenant_id}/invitations",
            get(list_invitations_handler).post(create_invitation_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/invitations/{invitation_id}/revoke",
            post(revoke_invitation_handler),
        )
        // 邀请链接管理（admin）。
        .route(
            "/v1/pm/tenants/{tenant_id}/invitation-links",
            get(list_links_handler).post(create_link_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/invitation-links/{link_id}/revoke",
            post(revoke_link_handler),
        )
        // 自助：预览 + 接受（未激活 principal 可调，仍需 bearer）。
        .route("/v1/pm/invitations/preview", get(preview_invitation_handler))
        .route("/v1/pm/invitations/accept", post(accept_invitation_handler))
        .route("/v1/pm/invitation-links/preview", get(preview_link_handler))
        .route("/v1/pm/invitation-links/accept", post(accept_link_handler))
}

/// 服务错误 → HTTP。稳定 machine message 直接透传（见模块头契约）。
fn api_err(err: TeamOnboardingError) -> Response {
    use axum::http::StatusCode;
    let message = err.to_string();
    let status = match &err {
        TeamOnboardingError::NotFound | TeamOnboardingError::InvitationNotFound => {
            StatusCode::NOT_FOUND
        }
        TeamOnboardingError::Validation(_) => StatusCode::BAD_REQUEST,
        TeamOnboardingError::Conflict(_)
        | TeamOnboardingError::Expired
        | TeamOnboardingError::Revoked
        | TeamOnboardingError::AlreadyAccepted
        | TeamOnboardingError::Exhausted => StatusCode::CONFLICT,
        TeamOnboardingError::Forbidden(_) => StatusCode::FORBIDDEN,
        TeamOnboardingError::Db(e) => {
            tracing::error!(error = %e, "team onboarding db error");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    (status, axum::Json(json!({ "message": message }))).into_response()
}

fn internal(e: axum::Error) -> Response {
    tracing::error!(error = %e, "team onboarding handler error");
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(json!({ "message": "internal error" })),
    )
        .into_response()
}

// ─────────────────────────────────────────────────────────── 建团

async fn create_team_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Json(req): Json<svc::CreateTeamRequest>,
) -> Response {
    match svc::create_team(
        &state.pool,
        &caller.audit_actor(),
        caller.principal_id,
        caller.tenant_id,
        req,
    )
    .await
    {
        Ok(team) => (axum::http::StatusCode::CREATED, Json(team)).into_response(),
        Err(err) => api_err(err),
    }
}

// ─────────────────────────────────────────────────────────── 团队档案

async fn team_profile_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
) -> Response {
    if guard_tenant(&caller, tenant_id).is_err() {
        return err_not_found("tenant not found");
    }
    match svc::team_profile(&state.pool, tenant_id).await {
        Ok(Some(team)) => (axum::http::StatusCode::OK, Json(team)).into_response(),
        Ok(None) => err_not_found("tenant not found"),
        Err(e) => internal(axum::Error::new(e)),
    }
}

async fn update_team_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    Json(req): Json<svc::UpdateTeamRequest>,
) -> Response {
    if guard_tenant(&caller, tenant_id).is_err() {
        return err_not_found("tenant not found");
    }
    if !caller.is_admin {
        return crate::team_llm_guard::err_forbidden("tenant admin required");
    }
    match svc::update_team_profile(&state.pool, &caller.audit_actor(), tenant_id, req).await {
        Ok(team) => (axum::http::StatusCode::OK, Json(team)).into_response(),
        Err(err) => api_err(err),
    }
}

// ─────────────────────────────────────────────────────────── 邮箱邀请

async fn create_invitation_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    Json(req): Json<svc::CreateInvitationRequest>,
) -> Response {
    if guard_tenant(&caller, tenant_id).is_err() {
        return err_not_found("tenant not found");
    }
    if !caller.is_admin {
        return crate::team_llm_guard::err_forbidden("tenant admin required");
    }
    match svc::create_invitation(&state.pool, &caller.audit_actor(), tenant_id, req).await {
        Ok(created) => (axum::http::StatusCode::CREATED, Json(created)).into_response(),
        Err(err) => api_err(err),
    }
}

async fn list_invitations_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
) -> Response {
    if guard_tenant(&caller, tenant_id).is_err() {
        return err_not_found("tenant not found");
    }
    if !caller.is_admin {
        return crate::team_llm_guard::err_forbidden("tenant admin required");
    }
    match svc::list_invitations(&state.pool, tenant_id).await {
        Ok(rows) => (axum::http::StatusCode::OK, Json(rows)).into_response(),
        Err(e) => internal(axum::Error::new(e)),
    }
}

async fn revoke_invitation_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Path((tenant_id, invitation_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if guard_tenant(&caller, tenant_id).is_err() {
        return err_not_found("tenant not found");
    }
    if !caller.is_admin {
        return crate::team_llm_guard::err_forbidden("tenant admin required");
    }
    match svc::revoke_invitation(&state.pool, &caller.audit_actor(), tenant_id, invitation_id)
        .await
    {
        Ok(()) => (
            axum::http::StatusCode::OK,
            Json(json!({ "id": invitation_id, "status": "REVOKED" })),
        )
            .into_response(),
        Err(err) => api_err(err),
    }
}

// ─────────────────────────────────────────────────────────── 邀请链接

async fn create_link_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    Json(req): Json<svc::CreateLinkRequest>,
) -> Response {
    if guard_tenant(&caller, tenant_id).is_err() {
        return err_not_found("tenant not found");
    }
    if !caller.is_admin {
        return crate::team_llm_guard::err_forbidden("tenant admin required");
    }
    match svc::create_invitation_link(&state.pool, &caller.audit_actor(), tenant_id, req).await {
        Ok(created) => (axum::http::StatusCode::CREATED, Json(created)).into_response(),
        Err(err) => api_err(err),
    }
}

async fn list_links_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
) -> Response {
    if guard_tenant(&caller, tenant_id).is_err() {
        return err_not_found("tenant not found");
    }
    if !caller.is_admin {
        return crate::team_llm_guard::err_forbidden("tenant admin required");
    }
    match svc::list_invitation_links(&state.pool, tenant_id).await {
        Ok(rows) => (axum::http::StatusCode::OK, Json(rows)).into_response(),
        Err(e) => internal(axum::Error::new(e)),
    }
}

async fn revoke_link_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Path((tenant_id, link_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if guard_tenant(&caller, tenant_id).is_err() {
        return err_not_found("tenant not found");
    }
    if !caller.is_admin {
        return crate::team_llm_guard::err_forbidden("tenant admin required");
    }
    match svc::revoke_invitation_link(&state.pool, &caller.audit_actor(), tenant_id, link_id).await
    {
        Ok(()) => (
            axum::http::StatusCode::OK,
            Json(json!({ "id": link_id, "status": "REVOKED" })),
        )
            .into_response(),
        Err(err) => api_err(err),
    }
}

// ─────────────────────────────────────────────────────────── 自助面

#[derive(Debug, serde::Deserialize)]
struct TokenQuery {
    token: String,
}

async fn preview_invitation_handler(
    State(state): State<AppState>,
    Query(q): Query<TokenQuery>,
) -> Response {
    match svc::preview_invitation(&state.pool, &q.token).await {
        Ok(Some(preview)) => (axum::http::StatusCode::OK, Json(preview)).into_response(),
        Ok(None) => err_not_found("invitation_not_found"),
        Err(e) => internal(axum::Error::new(e)),
    }
}

async fn accept_invitation_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Json(req): Json<svc::AcceptInvitationRequest>,
) -> Response {
    match svc::accept_invitation(
        &state.pool,
        &caller.audit_actor(),
        caller.principal_id,
        caller.tenant_id,
        req,
    )
    .await
    {
        Ok(outcome) => (axum::http::StatusCode::OK, Json(outcome)).into_response(),
        Err(err) => api_err(err),
    }
}

async fn preview_link_handler(State(state): State<AppState>, Query(q): Query<TokenQuery>) -> Response {
    match svc::preview_invitation_link(&state.pool, &q.token).await {
        Ok(Some(preview)) => (axum::http::StatusCode::OK, Json(preview)).into_response(),
        Ok(None) => err_not_found("invitation_not_found"),
        Err(e) => internal(axum::Error::new(e)),
    }
}

async fn accept_link_handler(
    State(state): State<AppState>,
    axum::Extension(caller): axum::Extension<AuthedPrincipal>,
    Json(req): Json<svc::AcceptLinkRequest>,
) -> Response {
    match svc::accept_invitation_link(
        &state.pool,
        &caller.audit_actor(),
        caller.principal_id,
        caller.tenant_id,
        &req.token,
    )
    .await
    {
        Ok(outcome) => (axum::http::StatusCode::OK, Json(outcome)).into_response(),
        Err(err) => api_err(err),
    }
}
