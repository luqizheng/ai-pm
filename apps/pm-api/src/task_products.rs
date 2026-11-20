//! `/v1/pm/tasks/{task_id}/products`：Task→Product 业务服务归属（PA07-IMP-004）。
//!
//! - GET    列出归属（含已撤回历史行，元素 `{product_id, product_name,
//!   product_status, scope, withdrawn, created_at, withdrawn_at}`）；
//! - POST   body `{product_id, scope?}` 建立归属（Product 必须处于 `normal`，
//!   否则 409 `product_abandoned`——错误码沿 Product 域约定，含义泛化为
//!   「不在可承接业务活动的状态」）；
//! - DELETE `?reason=` 必填，撤回为软删除（审查82 同款纪律：解除必须留原因）。
//!
//! 可见性沿用依赖端点：任务不存在/跨租户/非成员一律 404。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get},
    Json, Router,
};
use pm_application::task_products::{self, TaskProductError};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use pm_application::repo::GuardQueryRepo;
use crate::support::{require_project_visible, ApiError, ApiResult};

impl From<TaskProductError> for ApiError {
    fn from(e: TaskProductError) -> Self {
        match e {
            TaskProductError::TaskNotFound | TaskProductError::ProductNotFound => {
                ApiError::NotFound("not_found")
            }
            TaskProductError::ProductNotActive(code) => ApiError::Conflict(code),
            TaskProductError::Duplicate => ApiError::Conflict("TASK_PRODUCT_EXISTS"),
            TaskProductError::LinkNotFound => ApiError::NotFound("task_product_not_found"),
            TaskProductError::Db(err) => ApiError::Internal(err),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddTaskProductBody {
    pub product_id: Uuid,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WithdrawTaskProductQuery {
    reason: Option<String>,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/tasks/{task_id}/products",
            get(list_handler).post(add_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/products/{product_id}",
            delete(withdraw_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/product-acceptances",
            get(list_acceptances_handler).post(record_acceptance_handler),
        )
}

/// 任务的项目边界：不存在 / 跨租户 / 非成员 → 404。
async fn require_task_project(
    state: &AppState,
    task_id: Uuid,
    principal: &AuthedPrincipal,
) -> ApiResult<Uuid> {
    let project_id = guard_query_repo(&state.pool)
        .task_project_id(principal.tenant_id, task_id)
        .await?
        .ok_or(ApiError::NotFound("task_not_found"))?;
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    Ok(project_id)
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Vec<task_products::TaskProductLink>>> {
    require_task_project(&state, task_id, &principal).await?;
    let links = task_products::list_task_products(&state.pool, principal.tenant_id, task_id)
        .await?;
    Ok(Json(links))
}

async fn add_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<AddTaskProductBody>,
) -> ApiResult<Response> {
    require_task_project(&state, task_id, &principal).await?;
    let actor = principal.audit_actor();
    task_products::add_task_product(
        &state.pool,
        principal.tenant_id,
        task_id,
        body.product_id,
        body.scope.as_deref(),
        &actor,
    )
    .await?;
    Ok(StatusCode::CREATED.into_response())
}

async fn withdraw_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((task_id, product_id)): Path<(Uuid, Uuid)>,
    Query(params): Query<WithdrawTaskProductQuery>,
) -> ApiResult<Response> {
    require_task_project(&state, task_id, &principal).await?;
    let reason = params
        .reason
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .ok_or(ApiError::Text(
            StatusCode::BAD_REQUEST,
            "reason query parameter is required to withdraw a task-product link".to_string(),
        ))?;
    let actor = principal.audit_actor();
    task_products::withdraw_task_product(
        &state.pool,
        principal.tenant_id,
        task_id,
        product_id,
        &reason,
        &actor,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ───────────────────── Product 级验收（PA07-IMP-005，PA-07 §13）

#[derive(Debug, Deserialize)]
pub(crate) struct RecordAcceptanceBody {
    pub product_id: Uuid,
    #[serde(default)]
    pub task_revision_id: Option<Uuid>,
    /// `approved` / `rejected`。
    pub result: String,
    /// `common` / `product_specific`，缺省 product_specific。
    #[serde(default)]
    pub condition_scope: Option<String>,
    #[serde(default)]
    pub evidence_ref: Option<String>,
    /// `human` / `platform_flow`，缺省 human。
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

async fn list_acceptances_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Vec<task_products::ProductAcceptanceRow>>> {
    require_task_project(&state, task_id, &principal).await?;
    let rows = task_products::list_product_acceptances(
        &state.pool,
        principal.tenant_id,
        task_id,
    )
    .await?;
    Ok(Json(rows))
}

async fn record_acceptance_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<RecordAcceptanceBody>,
) -> ApiResult<Response> {
    require_task_project(&state, task_id, &principal).await?;
    let actor = principal.audit_actor();
    let acceptance_id = task_products::record_product_acceptance(
        &state.pool,
        principal.tenant_id,
        task_id,
        task_products::ProductAcceptanceInput {
            product_id: body.product_id,
            task_revision_id: body.task_revision_id,
            result: body.result,
            condition_scope: body
                .condition_scope
                .unwrap_or_else(|| "product_specific".to_string()),
            evidence_ref: body.evidence_ref,
            source: body.source.unwrap_or_else(|| "human".to_string()),
            note: body.note,
        },
        Some(principal.principal_id),
        &actor,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": acceptance_id })),
    )
        .into_response())
}
