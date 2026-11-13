//! `/v1/pm/acceptance-overview` 与 `/v1/pm/tasks/{task_id}/{review-findings,criterion-verdicts,review-approvals}`：
//! 项目交付验收工作台的只读端点（T-070 / `docs/tasks/ai-pm-delivery-acceptance-workbench-v1.md` §5.1 / §5.3）。
//!
//! - 第一屏：项目级聚合读模型——Q1 状态过滤 + Q2 后端统一判定徽标 + 动作区状态。
//! - 第二层：验收证据（findings / 逐条裁决 / 审批历史）。多仓交付清单复用
//!   `/v1/pm/tasks/{task_id}/delivery`（零改动）；验收动作复用
//!   `POST /v1/pm/review-approvals/{approval_id}/decision`（Q3，零新增写路径）。
//!
//! 可见性沿用 T-022：项目 / 任务不存在、跨租户或调用者非项目成员一律 404。

use axum::{
    extract::{Extension, Path, Query, State},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::repo::acceptance_read::AcceptanceReadRepo;
use crate::support::repos::acceptance_read_repo;
use crate::support::{require_project_visible, require_task_project, ApiError, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/acceptance-overview", get(overview_handler))
        .route(
            "/v1/pm/tasks/{task_id}/review-findings",
            get(review_findings_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/criterion-verdicts",
            get(criterion_verdicts_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/review-approvals",
            get(review_approvals_handler),
        )
}

/// `GET /v1/pm/acceptance-overview?project_id=`：待验收总表（§5.1）。
async fn overview_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<pm_application::AcceptanceOverview>> {
    let project_id = params
        .get("project_id")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let overview =
        pm_application::load_overview(&state.pool, project_id, principal.tenant_id).await?;
    Ok(Json(overview))
}

/// `GET /v1/pm/tasks/{task_id}/review-findings`：该任务全部 findings，`blocker` 置顶。
async fn review_findings_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_task_project(&state.pool, task_id, &principal).await?;
    let rows = acceptance_read_repo(&state.pool)
        .review_findings(task_id, project_id)
        .await?;
    Ok(Json(json!({
        "task_id": task_id,
        "project_id": project_id,
        "items": rows,
        "total": rows.len(),
    })))
}

/// 验收标准逐条裁决（当前快照）：标准正文 + 该标准**最新一次成功 review run** 的裁决。
///
/// `GET /v1/pm/tasks/{task_id}/criterion-verdicts`。
async fn criterion_verdicts_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_task_project(&state.pool, task_id, &principal).await?;
    let rows = acceptance_read_repo(&state.pool)
        .criterion_verdicts(task_id, project_id)
        .await?;
    Ok(Json(json!({
        "task_id": task_id,
        "project_id": project_id,
        "items": rows,
        "total": rows.len(),
    })))
}

/// `GET /v1/pm/tasks/{task_id}/review-approvals`：该任务审批历史（含 pending），
/// 与 `/v1/pm/review-approvals` 同一读模型的 task 级投影（§5.3）。
async fn review_approvals_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_task_project(&state.pool, task_id, &principal).await?;
    let rows = acceptance_read_repo(&state.pool)
        .task_approvals(task_id, project_id)
        .await?;
    Ok(Json(json!({
        "task_id": task_id,
        "project_id": project_id,
        "items": rows,
        "total": rows.len(),
    })))
}
