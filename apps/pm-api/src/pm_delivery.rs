//! `/v1/pm/tasks/{task_id}/delivery` 与 `/v1/pm/projects/{project_id}/delivery`：
//! 交付清单只读接口（T-041 / Slice D4；项目级聚合 S01-W60 / 缺陷 D-13）。
//!
//! 交付清单是 D4 的**事实陈述**：按 task × 契约仓列出基线、结果 commit、变更
//! 计数、测试证据与 `delivery_state`。它**不改状态机**——`delivered` 只表示仓库
//! 交付契约完备，不代表任务已验收或已完成（最终裁决归 D5）。
//!
//! 项目级聚合（总体计划 §9.1 W60 / §15.E「项目级交付可查」）只读投影
//! `delivery.v_task_delivery_manifest` + 任务清单 + pending 审批数，**不拥有业务
//! 状态、不重复维护领域状态**（I5 §9.3 同口径）。
//!
//! 可见性沿用 T-022：任务 / 项目不存在、跨租户或调用者非项目成员一律 404。

use axum::{
    extract::{Extension, Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, require_task_project, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/tasks/{task_id}/delivery", get(list_handler))
        .route(
            "/v1/pm/projects/{project_id}/delivery",
            get(project_handler),
        )
}

/// `GET /v1/pm/projects/{project_id}/delivery`：项目级交付聚合。
async fn project_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let items = pm_application::delivery::list_project_delivery(
        &state.pool,
        principal.tenant_id,
        project_id,
    )
    .await?;
    let count = |state_name: &str| {
        items
            .iter()
            .filter(|task| task.delivery_state == state_name)
            .count()
    };
    Ok(Json(serde_json::json!({
        "project_id": project_id,
        "items": items,
        "total": items.len(),
        "summary": {
            "delivered": count("delivered"),
            "partial": count("partial"),
            "missing": count("missing"),
            "no_contract": count("no_contract"),
        },
    })))
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = require_task_project(&state.pool, task_id, &principal).await?;
    let rows = pm_application::delivery::list_manifest(&state.pool, project_id, task_id).await?;
    Ok(Json(serde_json::json!({
        "task_id": task_id,
        "project_id": project_id,
        "items": rows,
        "total": rows.len(),
    })))
}
