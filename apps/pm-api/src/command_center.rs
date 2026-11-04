//! `GET /v1/pm/projects/{project_id}/command-center`（S01 · I5-C）。
//!
//! 项目总控台聚合读模型——**纯读**，不拥有任何业务状态。
//! 契约：`docs/s01-i5-read-model-contract.md`（I5-B 定稿）；事实：`docs/s01-i5-code-facts.md`。
//!
//! 规则：
//! - **权限前置**：项目可见性（同租户 + 项目成员）→ 不可见 404（不泄露存在性）；
//! - **按块降级**：某数据源不可用时该块 `status="unavailable"`，其余照常返回 200；
//! - **不返回 0 冒充「没有事项」**：不可用与空集合严格区分（契约 §2/§7.5）。

use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ApiResult, require_project_visible};

/// 活动流分页参数（契约 §1：只有「最近活动」块开放分页）。
#[derive(Debug, Deserialize)]
pub struct CommandCenterQuery {
    #[serde(default)]
    activity_limit: Option<i64>,
    #[serde(default)]
    activity_offset: Option<i64>,
}

const ACTIVITY_LIMIT_DEFAULT: i64 = 20;
const ACTIVITY_LIMIT_MAX: i64 = 100;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/v1/pm/projects/{project_id}/command-center",
        get(command_center_handler),
    )
}

async fn command_center_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<CommandCenterQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    // 权限前置：不可见一律 404（不泄露存在性）。
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let limit = query
        .activity_limit
        .unwrap_or(ACTIVITY_LIMIT_DEFAULT)
        .clamp(1, ACTIVITY_LIMIT_MAX);
    let offset = query.activity_offset.unwrap_or(0).max(0);

    let center = pm_application::command_center::load_command_center(
        &state.pool,
        principal.tenant_id,
        project_id,
        limit,
        offset,
    )
    .await;

    Ok(Json(json!({
        "project_id": center.project_id,
        "as_of": center.as_of,
        "blocks": {
            "project_summary": center.project_summary,
            "initialization_progress": center.initialization_progress,
            "work_summary": center.work_summary,
            "human_action_summary": center.human_action_summary,
            "risk_summary": center.risk_summary,
            "recent_activity": center.recent_activity,
        }
    })))
}
