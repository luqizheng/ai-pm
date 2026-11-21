//! `POST /v1/pm/projects/{project_id}/close`、`POST .../reopen`（P4-ORG Phase 5，
//! DR-PA-05）与 `POST /v1/pm/task-runs/{run_id}/cancel`（P2：cancel_run 外部入口）。
//!
//! 关闭：**关联 Products 的任意有效 Product Owner**（DR-PA-05②，2026-11-21
//! 裁决——Project Owner 已废止，Product 生命周期 Owner 延伸至 Project 生命周期）；
//! 单事务收口 queued Run（P1 条件取消）+ 在飞 Import 终态化（N-2）+ 名额释放（P4），
//! 事务外按取消协议处理活跃 Run（P2，reason=project_closed）。
//! 重开：同一守卫；closed → active，恢复原有写入与 AI 执行能力（基线 §10.3）。
//! Run 取消：项目成员可见即可取消自己的项目 Run；reason 透传审计。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use pm_application::archive::{close_project, is_project_lifecycle_manager, reopen_project};
use pm_application::run_executor;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use pm_application::repo::GuardQueryRepo;
use crate::support::require_project_visible;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/projects/{project_id}/close", post(close_handler))
        .route("/v1/pm/projects/{project_id}/reopen", post(reopen_handler))
        .route("/v1/pm/task-runs/{run_id}/cancel", post(cancel_run_handler))
}

#[derive(Debug, Deserialize)]
struct CloseBody {
    #[serde(default)]
    reason: Option<String>,
}

/// `POST /v1/pm/projects/{project_id}/close`。
async fn close_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    body: Option<Json<CloseBody>>,
) -> Response {
    // 生命周期守卫：关联 Products 的任意有效 Product Owner；其余 404（不可见即不存在）。
    match is_project_lifecycle_manager(&state.pool, caller.tenant_id, project_id, caller.principal_id).await {
        Ok(true) => {}
        Ok(false) => return not_found(),
        Err(err) => {
            tracing::error!(error = %err, "close manager check failed");
            return internal_error();
        }
    }

    let reason = body
        .and_then(|Json(body)| body.reason)
        .unwrap_or_else(|| "project_closed".to_string());
    let actor = caller.audit_actor();
    // 进程终止归 Worker 侧兜底路径（pm-api 与 Codex 不同机）；此处落终态 + 撤凭据。
    let executor = pm_application::run_dispatch::NoopExecutor;
    match close_project(
        &state.pool,
        &executor,
        &actor,
        caller.tenant_id,
        project_id,
        &reason,
    )
    .await
    {
        Ok(outcome) => Json(json!({
            "project_id": project_id,
            "status": "closed",
            "queued_runs_cancelled": outcome.queued_runs_cancelled,
            "imports_finalized": outcome.imports_finalized,
            "active_runs_cancelled": outcome.active_runs_cancelled,
        }))
        .into_response(),
        Err(pm_application::ArchiveError::AlreadyClosed) => (
            StatusCode::CONFLICT,
            Json(json!({ "code": "ALREADY_CLOSED", "message": "project is already closed" })),
        )
            .into_response(),
        Err(pm_application::ArchiveError::NotFound) => not_found(),
        Err(pm_application::ArchiveError::Db(err)) => {
            tracing::error!(error = %err, "close failed");
            internal_error()
        }
    }
}

/// `POST /v1/pm/projects/{project_id}/reopen`（同一守卫；closed → active）。
async fn reopen_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    match is_project_lifecycle_manager(&state.pool, caller.tenant_id, project_id, caller.principal_id).await {
        Ok(true) => {}
        Ok(false) => return not_found(),
        Err(err) => {
            tracing::error!(error = %err, "reopen manager check failed");
            return internal_error();
        }
    }
    let actor = caller.audit_actor();
    match reopen_project(&state.pool, &actor, caller.tenant_id, project_id).await {
        Ok(()) => Json(json!({ "project_id": project_id, "status": "active" })).into_response(),
        Err(pm_application::ArchiveError::AlreadyClosed) => (
            StatusCode::CONFLICT,
            Json(json!({ "code": "NOT_CLOSED", "message": "project is not closed" })),
        )
            .into_response(),
        Err(pm_application::ArchiveError::NotFound) => not_found(),
        Err(pm_application::ArchiveError::Db(err)) => {
            tracing::error!(error = %err, "reopen failed");
            internal_error()
        }
    }
}

#[derive(Debug, Deserialize)]
struct CancelRunBody {
    #[serde(default)]
    reason: Option<String>,
}

/// `POST /v1/pm/task-runs/{run_id}/cancel`：P2 外部取消入口。
async fn cancel_run_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(run_id): Path<Uuid>,
    body: Option<Json<CancelRunBody>>,
) -> Response {
    // Run → 项目 → 可见性（T-022）。
    let project_id: Option<Uuid> = guard_query_repo(&state.pool)
        .run_project_id(caller.tenant_id, run_id)
        .await
        .unwrap_or(None);
    let Some(project_id) = project_id else {
        return not_found();
    };
    if let Err(resp) = require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await
    {
        return resp.into_response();
    }

    let reason = body
        .and_then(|Json(body)| body.reason)
        .unwrap_or_else(|| "user_cancel".to_string());
    // 同上：Noop——终态与凭据在事务侧立即生效，进程由 Worker 兜底回收。
    let executor = pm_application::run_dispatch::NoopExecutor;
    match run_executor::cancel_run(&state.pool, &executor, run_id, &reason).await {
        Ok(true) => Json(json!({ "run_id": run_id, "status": "cancelled", "reason": reason }))
            .into_response(),
        // 不存在 / 不在可取消态（queued 归关闭路径、终态不可覆盖）。
        Ok(false) => (
            StatusCode::CONFLICT,
            Json(json!({
                "code": "NOT_CANCELLABLE",
                "message": "run is not in a cancellable state (claimed/running only)",
            })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "run cancel failed");
            internal_error()
        }
    }
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "message": "project not found" })),
    )
        .into_response()
}

fn internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "message": "internal error" })),
    )
        .into_response()
}
