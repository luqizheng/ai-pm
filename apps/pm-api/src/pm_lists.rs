//! `/v1/pm/*` 业务读端点（Slice A，前端 pm 经典页数据契约，见
//! `frontend/src/api/mock/fixtures.ts`）。统一 `{items,total,page,size}` 裸
//! payload、`project_id?` 过滤、`created_at DESC` 排序、page/size 分页；鉴权由
//! 路由外层的 `auth::require_bearer` 层承担（handlers 不做会话提取）。

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::routes::AppState;
use crate::support::{ApiResult, Paging, paging, project_filter};
use pm_application::rows::{
    ApprovalRow, AuditLogRow, DecisionRow, MilestoneRow, PrincipalRow, ProjectRow,
    RequirementRevisionRow, RequirementRow, TaskRow,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/principals", get(principals))
        .route("/v1/pm/projects", get(projects))
        .route("/v1/pm/requirements", get(requirements))
        .route("/v1/pm/tasks", get(tasks))
        .route("/v1/pm/milestones", get(milestones))
        .route("/v1/pm/approvals", get(approvals))
        .route("/v1/pm/decisions", get(decisions))
        .route("/v1/pm/audit-logs", get(audit_logs))
        .route(
            "/v1/pm/requirements/{requirement_id}/revisions",
            get(revisions).post(crate::pm_writes::create_revision_handler),
        )
        .route(
            "/v1/pm/requirements/{requirement_id}/revisions/{revision_id}/approve",
            post(crate::pm_writes::approve_revision_handler),
        )
        .route("/v1/pm/tasks", post(crate::pm_writes::create_task_handler))
        .route("/v1/pm/tasks/{task_id}", axum::routing::patch(crate::pm_writes::update_task_handler))
        .route(
            "/v1/pm/approvals/{approval_id}/decide",
            post(crate::pm_writes::decide_handler),
        )
}

async fn principals(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page: Paging = paging(&params);
    let items: Vec<PrincipalRow> = sqlx::query_as(
        "SELECT id, kind, display_name, is_active, created_at FROM core.principal \
         ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM core.principal")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn projects(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let items: Vec<ProjectRow> = sqlx::query_as(
        "SELECT id, customer_id, code, name, description, status, revision, created_at, updated_at \
         FROM core.project ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM core.project")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn requirements(
    State(state): State<AppState>,
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
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn tasks(
    State(state): State<AppState>,
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
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn milestones(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let items: Vec<MilestoneRow> = sqlx::query_as(
        "SELECT id, project_id, name, planned_at, status, revision \
         FROM delivery.milestone \
         WHERE ($1::uuid IS NULL OR project_id = $1) \
         ORDER BY planned_at NULLS LAST, name ASC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.milestone WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn approvals(
    State(state): State<AppState>,
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
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.approval_request WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

/// 决策中心读模型：审批请求 + 关联变更提案（左连接，提案缺失时字段为 null）。
async fn decisions(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    let items: Vec<DecisionRow> = sqlx::query_as(
        "SELECT a.id, a.project_id, a.status, a.required_role, a.decided_by, \
                a.decision_reason, a.decided_at, a.created_at, \
                p.id AS proposal_id, p.status AS proposal_status, p.target_type, \
                p.command_type, p.rationale, p.risk_level, p.evidence, \
                p.proposed_data, p.analysis_run_id \
         FROM governance.approval_request a \
         LEFT JOIN ai.change_proposal p ON p.id = a.proposal_id \
         WHERE ($1::uuid IS NULL OR a.project_id = $1) \
         ORDER BY a.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.approval_request \
         WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn audit_logs(
    State(state): State<AppState>,
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
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE ($1::uuid IS NULL OR project_id = $1)",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

/// 修订历史：`{items,total}`（version DESC；未知 requirement 不 404，返回空列表）。
async fn revisions(
    State(state): State<AppState>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let items: Vec<RequirementRevisionRow> = sqlx::query_as(
        "SELECT id, requirement_id, project_id, version, content, state, content_hash, \
                created_by, approved_by, created_at \
         FROM delivery.requirement_revision \
         WHERE requirement_id = $1 ORDER BY version DESC",
    )
    .bind(requirement_id)
    .fetch_all(&state.pool)
    .await?;
    let total = items.len() as i64;
    Ok(Json(serde_json::json!({ "items": items, "total": total })))
}
