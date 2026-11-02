//! `/v1/pm/tasks/{task_id}/delivery`：交付清单只读接口（T-041 / Slice D4）。
//!
//! 交付清单是 D4 的**事实陈述**：按 task × 契约仓列出基线、结果 commit、变更
//! 计数、测试证据与 `delivery_state`。它**不改状态机**——`delivered` 只表示仓库
//! 交付契约完备，不代表任务已验收或已完成（最终裁决归 D5）。
//!
//! 可见性沿用 T-022：任务不存在、跨租户或调用者非项目成员一律 404。

use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    routing::get,
};
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ApiResult, require_task_project};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/v1/pm/tasks/{task_id}/delivery",
        get(list_handler),
    )
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
