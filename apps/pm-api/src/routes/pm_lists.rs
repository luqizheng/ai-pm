//! `/v1/pm/*` 业务读端点（前端 pm 业务页的数据契约，见
//! frontend/src/api/mock/fixtures.ts）。统一 `{items,total,page,size}` 裸
//! payload，`project_id` 过滤、`created_at DESC` 排序、page/size 分页。
//! 全部要求 Bearer 会话。

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use super::auth::Authed;
use super::error::ApiResult;
use super::pagination::{paging, project_filter};

pub fn router() -> Router<PgPool> {
    Router::new()
        .route("/v1/pm/principals", get(principals))
        .route("/v1/pm/projects", get(projects))
        .route("/v1/pm/requirements", get(requirements))
        .route("/v1/pm/tasks", get(tasks))
        .route("/v1/pm/approvals", get(approvals))
        .route("/v1/pm/audit-logs", get(audit_logs))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PrincipalRow {
    pub id: Uuid,
    pub kind: String,
    pub display_name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RequirementRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub title: String,
    pub status: String,
    pub current_revision_id: Option<Uuid>,
    pub row_revision: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TaskRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub milestone_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub assignee_id: Option<Uuid>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ApprovalRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub proposal_id: Uuid,
    pub status: String,
    pub required_role: String,
    pub decided_by: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub actor_id: Uuid,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

async fn principals(
    State(pool): State<PgPool>,
    _auth: Authed,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let items: Vec<PrincipalRow> = sqlx::query_as(
        "SELECT id, kind, display_name, is_active, created_at FROM core.principal \
         ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM core.principal")
        .fetch_one(&pool)
        .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn projects(
    State(pool): State<PgPool>,
    _auth: Authed,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let items: Vec<ProjectRow> = sqlx::query_as(
        "SELECT id, customer_id, code, name, description, status, revision, created_at, updated_at \
         FROM core.project ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM core.project")
        .fetch_one(&pool)
        .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn requirements(
    State(pool): State<PgPool>,
    _auth: Authed,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let items: Vec<RequirementRow> = sqlx::query_as(
        "SELECT id, project_id, code, title, status, current_revision_id, row_revision, created_at \
         FROM delivery.requirement \
         WHERE ($1::uuid IS NULL OR project_id = $1) \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn tasks(
    State(pool): State<PgPool>,
    _auth: Authed,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let items: Vec<TaskRow> = sqlx::query_as(
        "SELECT id, project_id, milestone_id, title, description, status, priority, assignee_id, revision, created_at, updated_at \
         FROM delivery.task \
         WHERE ($1::uuid IS NULL OR project_id = $1) \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn approvals(
    State(pool): State<PgPool>,
    _auth: Authed,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let items: Vec<ApprovalRow> = sqlx::query_as(
        "SELECT id, project_id, proposal_id, status, required_role, decided_by, decision_reason, decided_at, created_at \
         FROM governance.approval_request \
         WHERE ($1::uuid IS NULL OR project_id = $1) \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.approval_request WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn audit_logs(
    State(pool): State<PgPool>,
    _auth: Authed,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let items: Vec<AuditLogRow> = sqlx::query_as(
        "SELECT id, project_id, actor_id, action, target_type, target_id, created_at \
         FROM governance.audit_log \
         WHERE ($1::uuid IS NULL OR project_id = $1) \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}
