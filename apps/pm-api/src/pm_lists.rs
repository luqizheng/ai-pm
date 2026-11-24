//! `/v1/pm/*` 业务读端点（Slice A，前端 pm 经典页数据契约，见
//! `frontend/src/api/mock/fixtures.ts`）。统一 `{items,total,page,size}` 裸
//! payload、`project_id?` 过滤、`created_at DESC` 排序、page/size 分页；鉴权由
//! 路由外层的 `auth::require_bearer` 层承担（handlers 不做会话提取）。
//!
//! 数据访问已收口到仓储（任务书批 B）：SQL 在
//! `pm_infrastructure::repo_lists::PgPmListRepo`，本模块零 SQL。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{
    customer_filter, paging, project_filter, require_project_visible, ApiError, ApiResult, Paging,
};
use pm_application::bootstrap::{
    bootstrap_project_for_customer_owned, bootstrap_project_owned, BootstrapError, BootstrapRequest,
};
use pm_application::repo::{ListPage, PmListRepo};
use pm_infrastructure::repo_lists::PgPmListRepo;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/pm/principals", get(principals))
        .route(
            "/v1/pm/projects",
            get(projects).post(create_project_handler),
        )
        .route("/v1/pm/requirements", get(requirements))
        .route("/v1/pm/tasks", get(tasks))
        .route("/v1/pm/milestones", get(milestones))
        .route("/v1/pm/approvals", get(approvals))
        .route("/v1/pm/decisions", get(decisions))
        .route("/v1/pm/audit-logs", get(audit_logs))
        .route(
            "/v1/pm/requirements/{requirement_id}/revisions",
            get(revisions),
        )
        .route("/v1/pm/tasks", post(crate::pm_writes::create_task_handler))
        .route(
            "/v1/pm/tasks/{task_id}",
            axum::routing::patch(crate::pm_writes::update_task_handler),
        )
        // DK-0 §7.4 / D-DK-8：整批 draft→ready 的人类确认入口（E5：每条任务
        // 恰一个 TaskReady 事件，走命令通道而非直写）。
        .route(
            "/v1/pm/projects/{project_id}/tasks/mark-ready",
            post(crate::pm_writes::mark_tasks_ready_handler),
        )
        .route(
            "/v1/pm/approvals/{approval_id}/decide",
            post(crate::pm_writes::decide_handler),
        )
}

fn repo(state: &AppState) -> PgPmListRepo {
    PgPmListRepo::new(state.pool.clone())
}

async fn principals(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page: Paging = paging(&params);
    let is_active = params
        .get("is_active")
        .and_then(|value| value.parse::<bool>().ok());
    let (items, total) = repo(&state)
        .list_principals(
            is_active,
            ListPage {
                size: page.size,
                offset: page.offset,
            },
        )
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
    let (items, total) = repo(&state)
        .list_projects(
            caller.tenant_id,
            caller.principal_id,
            customer_id,
            ListPage {
                size: page.size,
                offset: page.offset,
            },
        )
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
                Err(err) => return ApiError::from(err).into_response(),
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
            let description = body.description.trim();
            if !description.is_empty() {
                let _ = repo(&state)
                    .set_project_description(outcome.project_id, description)
                    .await;
            }
            match repo(&state)
                .find_project_row(caller.tenant_id, outcome.project_id)
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
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let (items, total) = repo(&state)
        .list_requirements(
            caller.tenant_id,
            caller.principal_id,
            project_id,
            ListPage {
                size: page.size,
                offset: page.offset,
            },
        )
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
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let (items, total) = repo(&state)
        .list_tasks(
            caller.tenant_id,
            caller.principal_id,
            project_id,
            ListPage {
                size: page.size,
                offset: page.offset,
            },
        )
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
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let (items, total) = repo(&state)
        .list_milestones(
            caller.tenant_id,
            caller.principal_id,
            project_id,
            ListPage {
                size: page.size,
                offset: page.offset,
            },
        )
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
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let (items, total) = repo(&state)
        .list_approvals(
            caller.tenant_id,
            caller.principal_id,
            project_id,
            ListPage {
                size: page.size,
                offset: page.offset,
            },
        )
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
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let (items, total) = repo(&state)
        .list_decisions(
            caller.tenant_id,
            caller.principal_id,
            project_id,
            ListPage {
                size: page.size,
                offset: page.offset,
            },
        )
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
        require_project_visible(
            &state.pool,
            project_id,
            caller.tenant_id,
            caller.principal_id,
        )
        .await?;
    }
    let (items, total) = repo(&state)
        .list_audit_logs(
            caller.tenant_id,
            caller.principal_id,
            project_id,
            ListPage {
                size: page.size,
                offset: page.offset,
            },
        )
        .await?;
    Ok(Json(page.to_response(items, total)))
}

/// 修订历史：`{items,total}`（version DESC；未知 requirement 不 404，返回空列表）。
async fn revisions(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(requirement_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = repo(&state)
        .list_requirement_revisions(caller.tenant_id, caller.principal_id, requirement_id)
        .await?;
    let total = items.len() as i64;
    Ok(Json(serde_json::json!({ "items": items, "total": total })))
}
