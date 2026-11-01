//! `/v1/pm/artifacts`：可验收「产物」的只读列表（AI 产物验收中心数据源）。
//!
//! 首版产物 = **需求修订**（`delivery.requirement_revision`）：AI 产出并经审批的
//! 需求正文版本。测试报告 / 文件 Diff / 行级评论在后端无数据源，前端显示空态，
//! 不虚构。裸 payload + `{items,total,page,size}`，`created_at DESC`。

use axum::{
    Json, Router,
    extract::{Extension, Query, State},
    routing::get,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ApiResult, paging, project_filter, require_project_visible};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/v1/pm/artifacts", get(artifacts))
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub(crate) struct ArtifactRow {
    pub id: Uuid,
    pub requirement_id: Uuid,
    pub requirement_code: String,
    pub requirement_title: String,
    pub version: i32,
    pub state: String,
    pub content_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub approved_by: Option<Uuid>,
}

async fn artifacts(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    if let Some(project_id) = project_id {
        require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id)
            .await?;
    }
    let items: Vec<ArtifactRow> = sqlx::query_as(
        "SELECT rv.id, rv.requirement_id, r.code AS requirement_code, \
                r.title AS requirement_title, rv.version, rv.state, rv.content_hash, \
                rv.created_at, rv.approved_by \
         FROM delivery.requirement_revision rv \
         JOIN delivery.requirement r ON r.id = rv.requirement_id \
         WHERE rv.tenant_id = $5 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = rv.project_id AND m.principal_id = $4) \
           AND ($1::uuid IS NULL OR rv.project_id = $1) \
         ORDER BY rv.created_at DESC, rv.version DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement_revision rv \
         WHERE rv.tenant_id = $3 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = rv.project_id AND m.principal_id = $2) \
           AND ($1::uuid IS NULL OR rv.project_id = $1)",
    )
    .bind(project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}