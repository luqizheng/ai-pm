//! `/v1/pm/task-runs`：任务执行 Run 的只读列表与单 Run 详情（spec §19.5 / §20；T-070 §5.2）。
//!
//! 自 Slice C 起 `TaskReady` handler 会为可调度任务创建 `delivery.task_run`；
//! 这里按项目列出 Run 及其失败摘要，供「AI 执行中心」展示真实执行数据。
//!
//! T-070 增补（纯读扩展，不改既有字段）：
//! - 列表补执行上下文列（`run_kind` / `agent_id` / `executor_kind` / `executor_session` /
//!   `lease_expires_at`）与两个可选过滤参数 `task_id`、`run_kind`（attempt_no 自 0026
//!   起按 run_kind 分轨编号，`delivery` / `review` 分轨展示）。
//! - `GET /v1/pm/task-runs/{run_id}`：租约 / 执行器 / 阶段时间线 + 本 Run 锁定的各仓基线
//!   （`task_run_repository`）。**不读日志内容**：日志在执行端磁盘，V1 只给
//!   `executor_session` 与 `workspace_path` 定位信息（设计稿 §二非目标）。
//!
//! 工具调用 / 日志 / 代码变化等执行细节需要 Codex/MCP 执行器（Slice D），当前
//! 无数据源，前端对这些区域显示空态而非虚构数据。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{
    paging, project_filter, repos::run_repo, require_project_visible, ApiError, ApiResult,
};
use pm_application::repo::runs::{Page, RunRepo, TaskRunDetail};
use serde_json::json;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/task-runs", get(task_runs))
        .route(
            "/v1/pm/task-run-cleanups/needs-human",
            get(cleanup_needs_human),
        )
        // S05 监控事件面（0013）：项目级汇总（静态段先于 {run_id} 匹配）。
        .route("/v1/pm/task-runs/summary", get(task_runs_summary))
        .route("/v1/pm/task-runs/{run_id}", get(task_run_detail))
        .route("/v1/pm/task-runs/{run_id}/events", get(task_run_events))
        // ACP-07（0054）：Deadline 过期重授权（UI 快捷入口；Lead/Approver 守卫
        // = 租户 admin 或项目 owner；独立审计 + 独立有效期，不改原 deadline）。
        .route(
            "/v1/pm/task-runs/{run_id}/execution-authorization",
            axum::routing::post(grant_authorization_handler).delete(revoke_authorization_handler),
        )
}

/// Project-scoped process-cleanup escalation list. A visible project is
/// required; tenant/project filtering is repeated in the application query.
async fn cleanup_needs_human(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = project_filter(&params).ok_or(ApiError::NotFound("project_not_found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50);
    let items = pm_application::run_cleanup::list_needs_human(
        &state.pool,
        caller.tenant_id,
        project_id,
        limit,
    )
    .await?;
    let count = items.len();
    Ok(Json(json!({ "items": items, "count": count })))
}

/// ACP-07 守卫：run 存在于调用者租户 + 授权者为其租户 admin 或项目 owner。
async fn ensure_authorization_manager(
    state: &AppState,
    principal: &AuthedPrincipal,
    run_id: Uuid,
) -> Result<(Uuid, Uuid, bool), Response> {
    let ctx = run_repo(&state.pool)
        .run_authorization_ctx(principal.tenant_id, principal.principal_id, run_id)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "code": "internal", "message": e.to_string() })),
            )
                .into_response()
        })?;
    let Some(ctx) = ctx else {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "code": "run_not_found", "message": "run not found" })),
        )
            .into_response());
    };
    let project_id = ctx.project_id;
    // 项目 owner 判定（core.project_member owner 行）。
    let is_project_owner = run_repo(&state.pool)
        .is_project_owner(project_id, principal.principal_id)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "code": "internal", "message": e.to_string() })),
            )
                .into_response()
        })?;
    let is_admin = ctx.team_role == Some("owner");
    if !(is_admin || is_project_owner) {
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            Json(json!({ "code": "FORBIDDEN", "message": "lead or approver required" })),
        )
            .into_response());
    }
    Ok((project_id, run_id, is_admin))
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct GrantAuthorizationBody {
    pub reason: String,
    pub new_deadline_at: chrono::DateTime<chrono::Utc>,
}

async fn grant_authorization_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
    Json(body): Json<GrantAuthorizationBody>,
) -> Response {
    let (project_id, _, _is_admin) =
        match ensure_authorization_manager(&state, &principal, run_id).await {
            Ok(v) => v,
            Err(resp) => return resp,
        };
    let actor = principal.audit_actor();
    let id = pm_application::acp06_07::grant_execution_authorization(
        &state.pool,
        principal.tenant_id,
        project_id,
        run_id,
        None,
        principal.principal_id,
        &body.reason,
        body.new_deadline_at,
        &actor,
    )
    .await;
    match id {
        Ok(id) => (
            StatusCode::CREATED,
            Json(json!({ "authorization_id": id, "run_id": run_id, "new_deadline_at": body.new_deadline_at })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": "internal", "message": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RevokeAuthorizationPath {
    pub authorization_id: Uuid,
}

async fn revoke_authorization_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
    Json(body): Json<RevokeAuthorizationPath>,
) -> Response {
    let (project_id, _, _) = match ensure_authorization_manager(&state, &principal, run_id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let actor = principal.audit_actor();
    match pm_application::acp06_07::revoke_execution_authorization(
        &state.pool,
        principal.tenant_id,
        project_id,
        run_id,
        body.authorization_id,
        &actor,
    )
    .await
    {
        Ok(revoked) => {
            Json(json!({ "authorization_id": body.authorization_id, "revoked": revoked }))
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": "internal", "message": e.to_string() })),
        )
            .into_response(),
    }
}

/// `run_kind` 过滤参数：只认 CHECK 枚举两值，非法值视为未传（与 `project_filter` 同口径）。
fn run_kind_filter(params: &HashMap<String, String>) -> Option<String> {
    params
        .get("run_kind")
        .filter(|v| matches!(v.as_str(), "delivery" | "review"))
        .cloned()
}

async fn task_runs(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let task_id = params.get("task_id").and_then(|v| Uuid::parse_str(v).ok());
    let run_kind = run_kind_filter(&params);
    if let Some(project_id) = project_id {
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let (items, total) = run_repo(&state.pool)
        .list_runs(
            caller.tenant_id,
            caller.principal_id,
            project_id,
            task_id,
            run_kind.as_deref(),
            Page {
                size: page.size,
                offset: page.offset,
            },
        )
        .await?;
    Ok(Json(page.to_response(items, total)))
}

/// `GET /v1/pm/task-runs/{run_id}`：run 不存在 / 跨租户 / 非成员 → 404（与 `/delivery` 同口径）。
async fn task_run_detail(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
) -> ApiResult<Json<TaskRunDetail>> {
    let repo = run_repo(&state.pool);
    let run = repo
        .find_run_detail(caller.tenant_id, caller.principal_id, run_id)
        .await?
        .ok_or(ApiError::NotFound("run_not_found"))?;
    let repositories = repo.run_repositories(run_id).await?;
    Ok(Json(TaskRunDetail { run, repositories }))
}

// ── S05 执行监控事件面（0013）──────────────────────────────────────────────

/// `GET /v1/pm/task-runs/{run_id}/events?type=`：run 事件时间线（occurred_at
/// 降序）。404 口径与 `task_run_detail` 一致（不存在 / 跨租户 / 非成员）；
/// `type` 只认 0013 CHECK 白名单，非法值视为未传。
async fn task_run_events(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let event_type = params
        .get("type")
        .filter(|v| pm_application::run_events::EVENT_TYPES.contains(&v.as_str()))
        .cloned();
    let repo = run_repo(&state.pool);
    if repo
        .run_visible(caller.tenant_id, caller.principal_id, run_id)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound("run_not_found"));
    }
    let (items, total) = repo
        .list_run_events(
            run_id,
            event_type.as_deref(),
            Page {
                size: page.size,
                offset: page.offset,
            },
        )
        .await?;
    Ok(Json(page.to_response(items, total)))
}

/// 项目内 Run 汇总（V2.0-G「汇总端点」最小面）：状态/结果分布 + 活跃数 +
/// 近 24h 事件数 + 最近事件时刻。`project_id` 必填（无跨项目模糊汇总）。
async fn task_runs_summary(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = project_filter(&params).ok_or(ApiError::NotFound("project_not_found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let repo = run_repo(&state.pool);
    let row = repo.run_summary(caller.tenant_id, project_id).await?;
    let ev = repo.run_event_summary(caller.tenant_id, project_id).await?;
    Ok(Json(json!({
        "project_id": project_id,
        "total": row.total,
        "by_status": row.by_status,
        "by_outcome": row.by_outcome,
        "active": row.active,
        "events_24h": ev.events_24h,
        "last_event_at": ev.last_event_at,
    })))
}
