//! S02 · 需求域页面级读面 REST 适配（评审队列 / 需求基线 / 变更影响分析）。
//!
//! 薄适配：只做（a）项目成员可见性闸、（b）业务裁决转交
//! `pm_application::requirement_read`、（c）错误映射。

use axum::{
    extract::{Extension, Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use pm_application::repo::GuardQueryRepo;
use crate::support::{require_project_visible, ApiResult};
use pm_application::requirement_read;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/requirement-review-queue",
            get(review_queue),
        )
        .route(
            "/v1/pm/projects/{project_id}/requirement-baseline",
            get(requirement_baseline),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/impact",
            get(requirement_impact),
        )
}

/// `GET /v1/pm/projects/{project_id}/requirement-review-queue`：待人工裁决的修订提案。
async fn review_queue(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let queue =
        requirement_read::project_review_queue(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!(queue)))
}

/// `GET /v1/pm/projects/{project_id}/requirement-baseline`：项目当前需求基线。
async fn requirement_baseline(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let baseline =
        requirement_read::project_requirement_baseline(&state.pool, caller.tenant_id, project_id)
            .await?;
    Ok(Json(serde_json::json!(baseline)))
}

/// `GET /v1/pm/requirements/{requirement_id}/impact`：变更影响分析。
async fn requirement_impact(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    // 需求的项目边界：不存在 / 跨租户 / 非成员一律 404。
    let project_id = guard_query_repo(&state.pool)
        .requirement_project_id(caller.tenant_id, requirement_id)
        .await?
        .ok_or(crate::support::ApiError::NotFound("requirement not found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let impact =
        requirement_read::requirement_impact(&state.pool, caller.tenant_id, requirement_id).await?;
    Ok(Json(serde_json::json!(impact)))
}
