//! S05 · 执行域读面 REST 适配（需要人工处理 / 态势概览 / 活动流 /
//! Run 附属事实 / 协作图 / Agent 名册）。
//!
//! 薄适配：只做（a）项目成员可见性闸、（b）业务裁决转交
//! `pm_application::execution_read`、（c）错误映射。

use axum::{
    extract::{Extension, Path, Query, State},
    routing::get,
    Json, Router,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use crate::support::{require_project_visible, require_task_project, ApiError, ApiResult};
use pm_application::execution_read;
use pm_application::repo::GuardQueryRepo;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/projects/{project_id}/waiting", get(project_waiting))
        .route(
            "/v1/pm/projects/{project_id}/execution-overview",
            get(project_execution_overview),
        )
        .route(
            "/v1/pm/projects/{project_id}/run-events",
            get(project_run_events),
        )
        .route("/v1/pm/projects/{project_id}/agents", get(project_agents))
        .route(
            "/v1/pm/tasks/{task_id}/collaboration",
            get(task_collaboration),
        )
        .route(
            "/v1/pm/task-runs/{run_id}/execution-authorization",
            get(list_run_authorizations),
        )
        .route("/v1/pm/task-runs/{run_id}/commands", get(list_run_commands))
        .route(
            "/v1/pm/task-runs/{run_id}/continuation",
            get(list_run_continuations),
        )
        .route("/v1/pm/task-runs/{run_id}/commits", get(list_run_commits))
}

/// Run 级端点的项目边界：Run 不存在 / 跨租户 / 非项目成员一律 404。
async fn require_run_project(
    state: &AppState,
    run_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<Uuid> {
    let project_id = guard_query_repo(&state.pool)
        .run_project_id(principal.tenant_id, run_id)
        .await?
        .ok_or(ApiError::NotFound("run_not_found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    Ok(project_id)
}

/// `GET /v1/pm/projects/{project_id}/waiting`：需要人工处理队列。
async fn project_waiting(
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
    let queue = execution_read::project_waiting(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!(queue)))
}

/// `GET /v1/pm/projects/{project_id}/execution-overview`：执行态势概览。
async fn project_execution_overview(
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
    let overview =
        execution_read::project_execution_overview(&state.pool, caller.tenant_id, project_id)
            .await?;
    Ok(Json(serde_json::json!(overview)))
}

/// `GET /v1/pm/projects/{project_id}/run-events`：项目级执行活动流。
async fn project_run_events(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let limit = params
        .get("limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(100);
    let items =
        execution_read::project_run_events(&state.pool, caller.tenant_id, project_id, limit)
            .await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// `GET /v1/pm/projects/{project_id}/agents`：Agent / 设备名册。
async fn project_agents(
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
    let items = execution_read::project_agents(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// `GET /v1/pm/tasks/{task_id}/collaboration`：多 Agent 协作图。
async fn task_collaboration(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_task_project(&state.pool, task_id, &caller).await?;
    let graph = execution_read::task_collaboration(&state.pool, caller.tenant_id, task_id).await?;
    Ok(Json(serde_json::json!(graph)))
}

/// `GET /v1/pm/task-runs/{run_id}/execution-authorization`：ACP-07 重授权台账。
async fn list_run_authorizations(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_run_project(&state, run_id, &caller).await?;
    let items =
        execution_read::list_run_authorizations(&state.pool, caller.tenant_id, run_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// `GET /v1/pm/task-runs/{run_id}/commands`：Worker 命令台账。
async fn list_run_commands(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_run_project(&state, run_id, &caller).await?;
    let items = execution_read::list_run_commands(&state.pool, caller.tenant_id, run_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// `GET /v1/pm/task-runs/{run_id}/continuation`：Continuation 链。
async fn list_run_continuations(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_run_project(&state, run_id, &caller).await?;
    let items =
        execution_read::list_run_continuations(&state.pool, caller.tenant_id, run_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// `GET /v1/pm/task-runs/{run_id}/commits`：Run 级逐仓交付清单。
async fn list_run_commits(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_run_project(&state, run_id, &caller).await?;
    let items = execution_read::list_run_commits(&state.pool, caller.tenant_id, run_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}
