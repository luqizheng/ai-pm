//! S04 · 任务域读面 REST 适配（单任务详情 / 项目依赖图 / 执行准备度 / 排期）。
//!
//! 薄适配：只做（a）项目成员可见性闸、（b）业务裁决转交
//! `pm_application::task_read`、（c）错误映射。业务语义零实现。

use axum::{
    extract::{Extension, Path, State},
    routing::{get, patch},
    Json, Router,
};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, require_task_project, ApiResult};
use pm_application::task_read::{self, TaskScheduleRequest};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/tasks/{task_id}", get(task_detail))
        .route("/v1/pm/tasks/{task_id}/schedule", patch(set_task_schedule))
        .route(
            "/v1/pm/projects/{project_id}/dependency-graph",
            get(dependency_graph),
        )
        .route(
            "/v1/pm/projects/{project_id}/execution-readiness",
            get(execution_readiness),
        )
}

/// `GET /v1/pm/tasks/{task_id}`：单任务完整读模型（S04-UI-03）。
///
/// 任务本体 + 依据追溯（需求/设计/ADR/基线）+ 依赖 + 仓库 + 验收标准
/// + 人工审批 + 审计历史——一次读完，页面不做 N+1。
async fn task_detail(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_task_project(&state.pool, task_id, &caller).await?;
    let detail = task_read::task_detail(&state.pool, caller.tenant_id, task_id).await?;
    Ok(Json(serde_json::json!(detail)))
}

/// `PATCH /v1/pm/tasks/{task_id}/schedule`：写生效排期（S04-UI-06 拖拽/改工期）。
async fn set_task_schedule(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<TaskScheduleRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_task_project(&state.pool, task_id, &caller).await?;
    let task = task_read::set_task_schedule(
        &state.pool,
        caller.tenant_id,
        task_id,
        body,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "task": task })))
}

/// `GET /v1/pm/projects/{project_id}/dependency-graph`：项目级依赖图（S04-UI-05）。
///
/// 节点 + 依赖边 + 分层 + 关键路径 + 并行组 + 环检测——分层/关键路径/并行组
/// 是投影（不是存储），随依赖边实时算出。
async fn dependency_graph(
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
    let graph =
        task_read::project_dependency_graph(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!(graph)))
}

/// `GET /v1/pm/projects/{project_id}/execution-readiness`：执行准备度（S04-UI-07）。
///
/// 六段检查（任务定义 / 依据追溯 / 依赖校验 / 工程准备 / 验收交付 / 人工批准）
/// 的逐项判定 + 三态汇总（ready / needs_attention / blocked）。
async fn execution_readiness(
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
    let readiness =
        task_read::project_execution_readiness(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(serde_json::json!(readiness)))
}
