//! `/v1/pm/projects/{project_id}/ai-roles*`：项目 AI Role 目录与启停
//! （V2 D-1=A，`docs/project-ai-team-v2/ai-pm-ai-team-kickoff-adjudication-pack-v1.md`
//! §二；迁移 0058）。
//!
//! - GET  `/ai-roles`                    —— 目录合并视图（模板 LEFT JOIN 项目行）；
//! - PUT  `/ai-roles/{role_id}/enabled`  —— body `{"enabled": bool}`；
//! - GET  `/ai-roles/{role_id}/versions` —— 不可变版本历史（append-only）。
//!
//! 权限：读 = 项目可见（非成员 404，沿用 T-022 口径）；写 = 租户 owner/admin
//! 或项目 owner（§12 同款矩阵，`guard_role_manager`）。`project_manager`
//! 不允许项目禁用（AT-D12/稿 §3.3）→ 409；跨租户/不存在一律 404。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, put},
    Json, Router,
};
use pm_application::ai_roles::{
    list_project_roles, list_role_versions, set_project_role_enabled, AiRoleError,
    ProjectAiRoleView,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

impl From<AiRoleError> for ApiError {
    fn from(e: AiRoleError) -> Self {
        match e {
            AiRoleError::NotFound => ApiError::NotFound("ai_role_not_found"),
            AiRoleError::ProjectNotFound => ApiError::NotFound("project not found"),
            AiRoleError::Undisableable(_) => ApiError::Conflict("AI_ROLE_NOT_DISABLEABLE"),
            AiRoleError::Validation(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            AiRoleError::Db(err) => ApiError::Internal(err),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetAiRoleEnabledBody {
    pub enabled: bool,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/projects/{project_id}/ai-roles", get(list_handler))
        .route(
            "/v1/pm/projects/{project_id}/ai-roles/{role_id}/enabled",
            put(set_enabled_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/ai-roles/{role_id}/versions",
            get(versions_handler),
        )
}

/// 项目边界：不存在 / 跨租户 → 404（不泄露存在性）。
async fn require_project_tenant(
    state: &AppState,
    project_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<()> {
    let project_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?;
    if project_tenant.as_ref() != Some(&principal.tenant_id) {
        return Err(ApiError::NotFound("project not found"));
    }
    Ok(())
}

/// 读守卫：项目可见（任务不存在、跨租户或调用者非成员一律 404）。
async fn require_read_access(
    state: &AppState,
    project_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<()> {
    require_project_tenant(state, project_id, principal).await?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
}

/// 写守卫：租户 owner/admin 或项目 owner（§12「Team Owner/Admin ✓」同款矩阵）。
async fn guard_role_manager(
    state: &AppState,
    principal: &AuthedPrincipal,
    project_id: Uuid,
) -> ApiResult<()> {
    require_project_tenant(state, project_id, principal).await?;
    if principal.is_admin {
        return Ok(());
    }
    match pm_application::project_member::is_project_manager(
        &state.pool,
        principal.tenant_id,
        project_id,
        principal.principal_id,
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(ApiError::Forbidden("project manager required")),
        Err(_) => Err(ApiError::NotFound("project not found")),
    }
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ProjectAiRoleView>>> {
    require_read_access(&state, project_id, &principal).await?;
    let roles = list_project_roles(&state.pool, principal.tenant_id, project_id).await?;
    Ok(Json(roles))
}

async fn set_enabled_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, role_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<SetAiRoleEnabledBody>,
) -> ApiResult<Json<ProjectAiRoleView>> {
    guard_role_manager(&state, &principal, project_id).await?;
    let actor = principal.audit_actor();
    let view = set_project_role_enabled(
        &state.pool,
        &actor,
        principal.tenant_id,
        project_id,
        role_id,
        body.enabled,
    )
    .await?;
    Ok(Json(view))
}

async fn versions_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, role_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    require_read_access(&state, project_id, &principal).await?;
    let versions =
        list_role_versions(&state.pool, principal.tenant_id, project_id, role_id).await?;
    Ok((StatusCode::OK, Json(versions)).into_response())
}
