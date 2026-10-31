//! `/v1/pm/task-runs`：任务执行 Run 的只读列表（spec §19.5 / §20）。
//!
//! 自 Slice C 起 `TaskReady` handler 会为可调度任务创建 `delivery.task_run`；
//! 这里按项目列出 Run 及其失败摘要，供「AI 执行中心」展示真实执行数据。
//! 工具调用 / 日志 / 代码变化等执行细节需要 Codex/MCP 执行器（Slice D），当前
//! 无数据源，前端对这些区域显示空态而非虚构数据。

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::routes::AppState;
use crate::support::{ApiResult, paging, project_filter};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/v1/pm/task-runs", get(task_runs))
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub(crate) struct TaskRunRow {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub attempt_no: i32,
    pub status: String,
    pub outcome: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub failure_category: Option<String>,
    pub failure_reason: Option<String>,
}

async fn task_runs(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let items: Vec<TaskRunRow> = sqlx::query_as(
        "SELECT r.id, r.task_id, t.title AS task_title, r.attempt_no, r.status, r.outcome, \
                r.created_at, r.started_at, r.finished_at, \
                f.category AS failure_category, f.reason_code AS failure_reason \
         FROM delivery.task_run r \
         JOIN delivery.task t ON t.id = r.task_id \
         LEFT JOIN delivery.task_run_failure f ON f.run_id = r.id \
         WHERE ($1::uuid IS NULL OR r.project_id = $1) \
         ORDER BY r.created_at DESC, r.attempt_no DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}