//! `GET /v1/pm/projects/{project_id}`、`PATCH /v1/pm/projects/{project_id}`
//! （P1 Project Settings · General）。
//!
//! 读：项目可见即可读（`project_visible`，T-022 租户 + 成员边界）。
//! 写：生命周期管理者守卫（`is_project_lifecycle_manager`，DR-PA-05②——
//! 与 close/reopen 同一执法点）；只允许 name/description，`code` 与生命周期
//! 状态不经此面变更。错误体沿用 `{message}` 约定。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use uuid::Uuid;

use pm_application::project_info::{
    get_project_info, update_project_info, ProjectInfoError, UpdateProjectInfoRequest,
};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/v1/pm/projects/{project_id}",
        get(get_handler).patch(update_handler),
    )
}

fn info_error(err: ProjectInfoError) -> Response {
    let status = match &err {
        ProjectInfoError::NotFound => StatusCode::NOT_FOUND,
        ProjectInfoError::Validation(_) => StatusCode::BAD_REQUEST,
        ProjectInfoError::Db(db) => {
            tracing::error!(error = %db, "project info database error");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    (
        status,
        Json(json!({ "message": err.to_string(), "error": "project_info_failed" })),
    )
        .into_response()
}

async fn visibility_guard(
    state: &AppState,
    caller: &AuthedPrincipal,
    project_id: Uuid,
) -> Result<(), Response> {
    match crate::support::project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "message": "project not found" })),
        )
            .into_response()),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "message": format!("visibility check failed: {err}") })),
        )
            .into_response()),
    }
}

async fn get_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = visibility_guard(&state, &caller, project_id).await {
        return resp;
    }
    match get_project_info(&state.pool, caller.tenant_id, project_id).await {
        Ok(info) => (StatusCode::OK, Json(info)).into_response(),
        Err(err) => info_error(err),
    }
}

async fn update_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<UpdateProjectInfoRequest>,
) -> Response {
    if let Err(resp) = visibility_guard(&state, &caller, project_id).await {
        return resp;
    }
    // 基本信息编辑 = 项目正式属性变更 → 生命周期管理者（DR-PA-05② 同款）。
    match pm_application::archive::is_project_lifecycle_manager(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "message": "only a product owner of this project can update project info",
                    "error": "lifecycle_manager_required",
                })),
            )
                .into_response()
        }
        Err(err) => {
            tracing::error!(error = %err, "project info manager check failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "internal error" })),
            )
                .into_response();
        }
    }
    let actor = caller.audit_actor();
    match update_project_info(
        &state.pool,
        &actor,
        caller.tenant_id,
        project_id,
        body,
    )
    .await
    {
        Ok(info) => (StatusCode::OK, Json(info)).into_response(),
        Err(err) => info_error(err),
    }
}
