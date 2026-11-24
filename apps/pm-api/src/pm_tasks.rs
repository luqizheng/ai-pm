//! S04 · 任务域读面 REST 适配（单任务详情 / 项目依赖图 / 执行准备度 / 排期）。
//!
//! 薄适配：只做（a）项目成员可见性闸、（b）业务裁决转交
//! `pm_application::task_read`、（c）错误映射。业务语义零实现。

use axum::{
    extract::{Extension, Path, State},
    response::IntoResponse,
    routing::{get, patch, post},
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
        .route("/v1/pm/tasks/{task_id}/revisions", get(task_revisions))
        .route("/v1/pm/tasks/{task_id}/schedule", patch(set_task_schedule))
        // B06-D2：人工任务直接完成（真人唯一通道；状态机 Ready→Completed 边）
        .route("/v1/pm/tasks/{task_id}/transitions", post(task_transition))
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

/// `GET /v1/pm/tasks/{task_id}/revisions`：Revision 历史时间线（PA-07 D 画布）。
/// 时间线 + 陈旧活跃 Run 警告数据；任务不存在/跨租户 → 404。
async fn task_revisions(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_task_project(&state.pool, task_id, &caller).await?;
    let Some((revisions, stale)) =
        pm_application::task_revisions::list_task_revisions(&state.pool, caller.tenant_id, task_id)
            .await?
    else {
        return Err(crate::support::ApiError::NotFound("task_not_found"));
    };
    Ok(Json(serde_json::json!({
        "items": revisions,
        "stale_active_runs": stale,
    })))
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

/// B06-D2：`POST /v1/pm/tasks/{task_id}/transitions`——人工任务直接完成。
///
/// 真人唯一通道（AI 主体 403）；body：`{ to: "completed", reason, expected_revision? }`。
/// 理由必填落审计；状态机侧 `Ready → Completed` 边以 manual_completion 事实守卫。
#[derive(serde::Deserialize)]
struct TaskTransitionBody {
    to: String,
    reason: String,
    #[serde(default)]
    expected_revision: Option<i64>,
}

async fn task_transition(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<TaskTransitionBody>,
) -> ApiResult<axum::response::Response> {
    if body.to != "completed" {
        return Err(crate::support::ApiError::BadRequest(
            "only `completed` is supported on this endpoint",
        ));
    }
    if body.reason.trim().is_empty() {
        return Err(crate::support::ApiError::BadRequest(
            "completion reason is required (human completion must be accountable)",
        ));
    }
    require_task_project(&state.pool, task_id, &caller).await?;
    match pm_application::command_executor::manual_complete_task(
        &state.pool,
        caller.tenant_id,
        task_id,
        body.reason,
        body.expected_revision,
        caller.audit_actor(),
    )
    .await
    {
        Ok(revision) => Ok(axum::Json(serde_json::json!({
            "task_id": task_id,
            "status": "completed",
            "revision": revision,
        }))
        .into_response()),
        Err(pm_application::CommandError::RevisionConflict { expected, actual }) => Ok((
            axum::http::StatusCode::CONFLICT,
            axum::Json(serde_json::json!({
                "error": "revision_conflict",
                "expected_revision": expected,
                "actual_revision": actual,
            })),
        )
            .into_response()),
        Err(pm_application::CommandError::Validation(detail)) => Ok((
            axum::http::StatusCode::CONFLICT,
            axum::Json(serde_json::json!({ "error": "transition_rejected", "detail": detail })),
        )
            .into_response()),
        Err(err) => {
            tracing::error!(error = %err, "manual task completion failed");
            Err(crate::support::ApiError::Internal(
                sqlx::Error::Configuration("manual completion failed".into()),
            ))
        }
    }
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
