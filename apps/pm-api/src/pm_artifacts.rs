//! `/v1/pm/artifacts`：可验收「产物」的只读列表（AI 产物验收中心数据源）。
//!
//! 首版产物 = **需求修订**（`delivery.requirement_revision`）：AI 产出并经审批的
//! 需求正文版本。测试报告 / 文件 Diff / 行级评论在后端无数据源，前端显示空态，
//! 不虚构。裸 payload + `{items,total,page,size}`，`created_at DESC`。

use axum::{
    extract::{Extension, Query, State},
    routing::get,
    Json, Router,
};
use std::collections::HashMap;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::repo::acceptance_read::{AcceptanceReadRepo, Page};
use crate::support::repos::acceptance_read_repo;
use crate::support::{paging, project_filter, require_project_visible, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/v1/pm/artifacts", get(artifacts))
}

async fn artifacts(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    if let Some(project_id) = project_id {
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let (items, total) = acceptance_read_repo(&state.pool)
        .list_artifacts(
            caller.tenant_id,
            caller.principal_id,
            project_id,
            Page { size: page.size, offset: page.offset },
        )
        .await?;
    Ok(Json(page.to_response(items, total)))
}
