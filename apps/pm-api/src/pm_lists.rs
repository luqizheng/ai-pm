//! `/v1/pm/*` 业务读端点（Slice A，前端 pm 经典页数据契约，见
//! `frontend/src/api/mock/fixtures.ts`）。统一 `{items,total,page,size}` 裸
//! payload、`project_id?` 过滤、`created_at DESC` 排序、page/size 分页；鉴权由
//! 路由外层的 `auth::require_bearer` 层承担（handlers 不做会话提取）。

use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{
    ApiResult, Paging, customer_filter, paging, project_filter, require_project_visible,
};
use pm_application::bootstrap::{
    BootstrapError, BootstrapRequest, bootstrap_project_for_customer_owned, bootstrap_project_owned,
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
        .route("/v1/pm/requirements/{requirement_id}/revisions", get(revisions))
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
    let is_active = params.get("is_active").and_then(|value| value.parse::<bool>().ok());
    // 平台专用 service principal（`ai-pm-dispatch`，0015 幂等种子）是审计兜底
    // 身份，不属于可指派的人员/代理列表，故不对外暴露。
    let items: Vec<PrincipalRow> = sqlx::query_as(
        "SELECT id, kind, display_name, is_active, created_at FROM core.principal \
          WHERE kind <> 'service' AND ($3::bool IS NULL OR is_active = $3) \
          ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page.size)
    .bind(page.offset)
    .bind(is_active)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.principal \
          WHERE kind <> 'service' AND ($1::bool IS NULL OR is_active = $1)",
    )
    .bind(is_active)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn projects(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let customer_id = customer_filter(&params);
    // customer_id 是 NOT NULL 外键，INNER JOIN 不会丢行；COALESCE 只为让行结构
    // 保持 String（前端不需要区分「有客户但没名字」）。可见性（T-022）：只列
    // 调用者在 `core.project_member` 里的项目。
    let items: Vec<ProjectRow> = sqlx::query_as(
        "SELECT p.id, p.customer_id, COALESCE(c.name, '') AS customer_name, \
                COALESCE(c.status, 'active') AS customer_status, \
                p.code, p.name, p.description, p.status, p.revision, p.created_at, p.updated_at \
         FROM core.project p JOIN core.customer c ON c.id = p.customer_id \
         WHERE p.tenant_id = $5 \
           AND p.status <> 'deleted' \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = p.id AND m.principal_id = $4) \
           AND ($1::uuid IS NULL OR p.customer_id = $1) \
         ORDER BY p.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(customer_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project p \
          WHERE p.tenant_id = $3 \
            AND p.status <> 'deleted' \
            AND EXISTS (SELECT 1 FROM core.project_member m \
                         WHERE m.project_id = p.id AND m.principal_id = $2) \
            AND ($1::uuid IS NULL OR p.customer_id = $1)",
    )
    .bind(customer_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
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
    Extension(caller): Extension<AuthedPrincipal>,
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
    // T-022：调用者必须是 owner 成员，且只能挂到自己可见的客户上。
    let request = BootstrapRequest {
        project_code: code.to_string(),
        project_name: name.to_string(),
        tenant_id: caller.tenant_id,
    };
    let outcome = match body.customer_id {
        Some(customer_id) => {
            match crate::customers::customer_visible_to(
                &state.pool,
                caller.tenant_id,
                customer_id,
                caller.principal_id,
            )
            .await
            {
                Ok(true) => {}
                // 与 `BootstrapError::CustomerNotFound` 同一响应：不可见客户等同
                // 不存在，保持建项接口既有的 400 契约。
                Ok(false) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "message": format!("customer `{customer_id}` not found or archived")
                        })),
                    )
                        .into_response()
                }
                Err(err) => return crate::support::ApiError::from(err).into_response(),
            }
            bootstrap_project_for_customer_owned(
                &state.pool,
                request,
                customer_id,
                caller.principal_id,
                None,
            )
            .await
        }
        None => bootstrap_project_owned(&state.pool, request, caller.principal_id).await,
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
                 WHERE p.id = $1 AND p.tenant_id = $2",
            )
            .bind(outcome.project_id)
            .bind(caller.tenant_id)
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
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let project_id = project_filter(&params);
    if let Some(project_id) = project_id {
        require_project_visible(&state.pool, project_id, caller.tenant_id, caller.principal_id)
            .await?;
    }
    let items: Vec<RequirementRow> = sqlx::query_as(
        "SELECT r.id, r.project_id, r.code, r.title, r.lifecycle_status, r.current_revision_id, \
                r.row_revision, r.created_at \
         FROM delivery.requirement r \
         WHERE r.tenant_id = $5 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = r.project_id AND m.principal_id = $4) \
           AND ($1::uuid IS NULL OR r.project_id = $1) \
         ORDER BY r.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement r \
          WHERE r.tenant_id = $3 \
            AND EXISTS (SELECT 1 FROM core.project_member m \
                         WHERE m.project_id = r.project_id AND m.principal_id = $2) \
            AND ($1::uuid IS NULL OR r.project_id = $1)",
    )
    .bind(project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn tasks(
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
    let items: Vec<TaskRow> = sqlx::query_as(
        "SELECT t.id, t.project_id, t.milestone_id, t.title, t.description, t.status, \
                t.priority, t.assignee_id, t.revision, t.created_at, t.updated_at \
         FROM delivery.task t \
         WHERE t.tenant_id = $5 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = t.project_id AND m.principal_id = $4) \
           AND ($1::uuid IS NULL OR t.project_id = $1) \
         ORDER BY t.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task t \
          WHERE t.tenant_id = $3 \
            AND EXISTS (SELECT 1 FROM core.project_member m \
                         WHERE m.project_id = t.project_id AND m.principal_id = $2) \
            AND ($1::uuid IS NULL OR t.project_id = $1)",
    )
    .bind(project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn milestones(
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
    let items: Vec<MilestoneRow> = sqlx::query_as(
        "SELECT m.id, m.project_id, m.name, m.planned_at, m.status, m.revision \
         FROM delivery.milestone m \
         WHERE m.tenant_id = $5 \
           AND EXISTS (SELECT 1 FROM core.project_member pm \
                        WHERE pm.project_id = m.project_id AND pm.principal_id = $4) \
           AND ($1::uuid IS NULL OR m.project_id = $1) \
         ORDER BY m.planned_at NULLS LAST, m.name ASC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.milestone m \
          WHERE m.tenant_id = $3 \
            AND EXISTS (SELECT 1 FROM core.project_member pm \
                         WHERE pm.project_id = m.project_id AND pm.principal_id = $2) \
            AND ($1::uuid IS NULL OR m.project_id = $1)",
    )
    .bind(project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn approvals(
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
    let items: Vec<ApprovalRow> = sqlx::query_as(
        "SELECT a.id, a.project_id, a.proposal_id, a.status, a.required_role, a.decided_by, \
                a.decision_reason, a.decided_at, a.created_at \
         FROM governance.approval_request a \
         WHERE a.tenant_id = $5 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = a.project_id AND m.principal_id = $4) \
           AND ($1::uuid IS NULL OR a.project_id = $1) \
         ORDER BY a.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.approval_request a \
          WHERE a.tenant_id = $3 \
            AND EXISTS (SELECT 1 FROM core.project_member m \
                         WHERE m.project_id = a.project_id AND m.principal_id = $2) \
            AND ($1::uuid IS NULL OR a.project_id = $1)",
    )
    .bind(project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

/// 决策中心读模型：审批请求 + 关联变更提案（左连接，提案缺失时字段为 null）。
async fn decisions(
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
    let items: Vec<DecisionRow> = sqlx::query_as(
        "SELECT a.id, a.project_id, a.status, a.required_role, a.decided_by, \
                a.decision_reason, a.decided_at, a.created_at, \
                p.id AS proposal_id, p.status AS proposal_status, p.target_type, \
                p.command_type, p.rationale, p.risk_level, p.evidence, \
                p.proposed_data, p.analysis_run_id \
         FROM governance.approval_request a \
         LEFT JOIN ai.change_proposal p ON p.id = a.proposal_id \
         WHERE a.tenant_id = $5 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = a.project_id AND m.principal_id = $4) \
           AND ($1::uuid IS NULL OR a.project_id = $1) \
         ORDER BY a.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.approval_request a \
         WHERE a.tenant_id = $3 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = a.project_id AND m.principal_id = $2) \
           AND ($1::uuid IS NULL OR a.project_id = $1)",
    )
    .bind(project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

async fn audit_logs(
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
    let items: Vec<AuditLogRow> = sqlx::query_as(
        "SELECT g.id, g.project_id, g.actor_id, g.action, g.target_type, g.target_id, g.created_at \
         FROM governance.audit_log g \
         WHERE g.tenant_id = $5 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = g.project_id AND m.principal_id = $4) \
           AND ($1::uuid IS NULL OR g.project_id = $1) \
         ORDER BY g.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(page.size)
    .bind(page.offset)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log g \
          WHERE g.tenant_id = $3 \
            AND EXISTS (SELECT 1 FROM core.project_member m \
                         WHERE m.project_id = g.project_id AND m.principal_id = $2) \
            AND ($1::uuid IS NULL OR g.project_id = $1)",
    )
    .bind(project_id)
    .bind(caller.principal_id)
    .bind(caller.tenant_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(page.to_response(items, total)))
}

/// 修订历史：`{items,total}`（version DESC；未知 requirement 不 404，返回空列表）。
async fn revisions(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let items: Vec<RequirementRevisionRow> = sqlx::query_as(
        "SELECT rv.id, rv.requirement_id, rv.project_id, rv.version, rv.content, rv.state, \
                rv.content_hash, rv.created_by, rv.approved_by, rv.created_at \
         FROM delivery.requirement_revision rv \
         WHERE rv.requirement_id = $1 AND rv.tenant_id = $2 \
           AND EXISTS (SELECT 1 FROM core.project_member m \
                        WHERE m.project_id = rv.project_id AND m.principal_id = $3) \
         ORDER BY rv.version DESC",
    )
    .bind(requirement_id)
    .bind(caller.tenant_id)
    .bind(caller.principal_id)
    .fetch_all(&state.pool)
    .await?;
    let total = items.len() as i64;
    Ok(Json(serde_json::json!({ "items": items, "total": total })))
}
