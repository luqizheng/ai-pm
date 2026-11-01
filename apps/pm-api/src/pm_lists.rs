//! `/v1/pm/*` 业务读端点（Slice A，前端 pm 经典页数据契约，见
//! `frontend/src/api/mock/fixtures.ts`）。统一 `{items,total,page,size}` 裸
//! payload、`project_id?` 过滤、`created_at DESC` 排序、page/size 分页；鉴权由
//! 路由外层的 `auth::require_bearer` 层承担（handlers 不做会话提取）。

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::routes::AppState;
use crate::support::{ApiResult, Paging, customer_filter, paging, project_filter};
use pm_application::bootstrap::{
    BootstrapError, BootstrapRequest, bootstrap_project, bootstrap_project_for_customer,
};
use pm_application::rows::{
    ApprovalRow, AuditLogRow, DecisionRow, MilestoneRow, PrincipalRow, ProjectRow,
    RequirementRevisionRow, RequirementRow, TaskRow,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/principals", get(principals))
        .route("/v1/pm/projects", get(projects).post(create_project_handler))
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
    // 平台专用 service principal（`ai-pm-dispatch`，0015 幂等种子）是审计兜底
    // 身份，不属于可指派的人员/代理列表，故不对外暴露。
    let items: Vec<PrincipalRow> = sqlx::query_as(
        "SELECT id, kind, display_name, is_active, created_at FROM core.principal \
          WHERE kind <> 'service' \
          ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.principal WHERE kind <> 'service'")
            .fetch_one(&state.pool)
            .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn projects(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let customer_id = customer_filter(&params);
    // customer_id 是 NOT NULL 外键，INNER JOIN 不会丢行；COALESCE 只为让行结构
    // 保持 String（前端不需要区分「有客户但没名字」）。
    let items: Vec<ProjectRow> = sqlx::query_as(
        "SELECT p.id, p.customer_id, COALESCE(c.name, '') AS customer_name, \
                COALESCE(c.status, 'active') AS customer_status, \
                p.code, p.name, p.description, p.status, p.revision, p.created_at, p.updated_at \
         FROM core.project p JOIN core.customer c ON c.id = p.customer_id \
         WHERE ($1::uuid IS NULL OR p.customer_id = $1) \
         ORDER BY p.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(customer_id)
    .bind(page.size)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project WHERE ($1::uuid IS NULL OR customer_id = $1)",
    )
    .bind(customer_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

#[derive(Debug, serde::Deserialize)]
struct CreateProjectBody {
    code: String,
    name: String,
    #[serde(default)]
    description: String,
    /// 0017：挂到已有客户主数据。缺省时退回旧行为（按 code 隐式造 placeholder 客户）。
    #[serde(default)]
    customer_id: Option<Uuid>,
}

/// `POST /v1/pm/projects`：建项目并自动落 5 人编制（owner/ai_pm/architect/developer/tester）。
async fn create_project_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateProjectBody>,
) -> Response {
    let code = body.code.trim();
    let name = body.name.trim();
    if code.is_empty() || name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "message": "project code and name are required" })),
        )
            .into_response();
    }
    let request = BootstrapRequest {
        project_code: code.to_string(),
        project_name: name.to_string(),
    };
    let outcome = match body.customer_id {
        Some(customer_id) => {
            bootstrap_project_for_customer(&state.pool, request, customer_id, None).await
        }
        None => bootstrap_project(&state.pool, request).await,
    };
    match outcome {
        Ok(outcome) => {
            if !body.description.trim().is_empty() {
                let _ = sqlx::query("UPDATE core.project SET description = $2 WHERE id = $1")
                    .bind(outcome.project_id)
                    .bind(body.description.trim())
                    .execute(&state.pool)
                    .await;
            }
            match sqlx::query_as::<_, ProjectRow>(
                "SELECT p.id, p.customer_id, COALESCE(c.name, '') AS customer_name, \
                        COALESCE(c.status, 'active') AS customer_status, \
                        p.code, p.name, p.description, p.status, p.revision, p.created_at, p.updated_at \
                 FROM core.project p JOIN core.customer c ON c.id = p.customer_id \
                 WHERE p.id = $1",
            )
            .bind(outcome.project_id)
            .fetch_one(&state.pool)
            .await
            {
                Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
                Err(err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "message": err.to_string() })),
                )
                    .into_response(),
            }
        }
        Err(BootstrapError::Duplicate(code)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "message": format!("project code `{code}` already exists") })),
        )
            .into_response(),
        Err(BootstrapError::CustomerNotFound(customer_id)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "message": format!("customer `{customer_id}` not found or archived")
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "message": err.to_string() })),
        )
            .into_response(),
    }
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
