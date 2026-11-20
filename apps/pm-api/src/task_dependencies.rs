//! `/v1/pm/tasks/{task_id}/dependencies`：Task 前置依赖的读写端点（T-024）。
//!
//! - GET  列出上游/下游（元素 `{id, title, status}`）；
//! - POST body `{depends_on_task_id}`，为下游增加一条指向前置的边；
//! - DELETE 幂等删除边（不存在也返回 204）。
//!
//! 可见性沿用 T-022：任务不存在、跨租户或调用者非项目成员一律 404，不泄露存在性。
//! 写路径的租户前置校验在这里完成；应用层只看 `project_id`（组合外键已保证同租户）。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get},
    Json, Router,
};
use pm_application::task_dependency::{self, DependencyError, DependencyGraph};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use pm_application::repo::GuardQueryRepo;
use crate::support::{require_project_visible, ApiError, ApiResult};

impl From<DependencyError> for ApiError {
    fn from(e: DependencyError) -> Self {
        match e {
            DependencyError::TaskNotFound => ApiError::NotFound("task_not_found"),
            DependencyError::SelfDependency => ApiError::Conflict("SELF_DEPENDENCY"),
            DependencyError::TaskNotPending(_) => ApiError::Conflict("TASK_NOT_PENDING"),
            DependencyError::Duplicate => ApiError::Conflict("DUPLICATE_DEPENDENCY"),
            DependencyError::CrossProject => ApiError::Conflict("CROSS_PROJECT_DEPENDENCY"),
            DependencyError::Cycle => ApiError::Conflict("DEPENDENCY_CYCLE"),
            // PA07-IMP-003：条件/交付物引用非法是调用方输入错误（400）。
            DependencyError::InvalidCondition(message) => {
                ApiError::Text(StatusCode::BAD_REQUEST, message)
            }
            DependencyError::Db(err) => ApiError::Internal(err),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddDependencyBody {
    pub depends_on_task_id: Uuid,
    /// PA07-IMP-003（审查75）：满足条件，缺省 completed；三值封闭枚举。
    #[serde(default)]
    pub required_condition: Option<String>,
    /// deliverable_approved 必填：{id} 或 {path}。
    #[serde(default)]
    pub required_deliverable_ref: Option<serde_json::Value>,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/tasks/{task_id}/dependencies",
            get(list_handler).post(add_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/dependencies/{depends_on_task_id}",
            delete(remove_handler),
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
) -> ApiResult<Json<DependencyGraph>> {
    require_task_project(&state, task_id, &principal).await?;
    let graph =
        task_dependency::list_dependencies(&state.pool, principal.tenant_id, task_id).await?;
    Ok(Json(graph))
}

async fn add_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<AddDependencyBody>,
) -> ApiResult<Response> {
    require_task_project(&state, task_id, &principal).await?;
    let actor = principal.audit_actor();
    let graph = task_dependency::add_dependency_with(
        &state.pool,
        task_id,
        body.depends_on_task_id,
        task_dependency::AddDependencyOptions {
            required_condition: body.required_condition,
            required_deliverable_ref: body.required_deliverable_ref,
        },
        &actor,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(graph)).into_response())
}

/// 删边（审查82）：`?reason=` 必填——解除依赖必须记录原因，不是临时忽略门禁。
async fn remove_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((task_id, depends_on_task_id)): Path<(Uuid, Uuid)>,
    axum::extract::Query(params): axum::extract::Query<RemoveDependencyQuery>,
) -> ApiResult<Response> {
    require_task_project(&state, task_id, &principal).await?;
    let reason = params
        .reason
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .ok_or(ApiError::Text(
            StatusCode::BAD_REQUEST,
            "reason query parameter is required to remove a dependency".to_string(),
        ))?;
    let actor = principal.audit_actor();
    task_dependency::remove_dependency_with_reason(
        &state.pool,
        task_id,
        depends_on_task_id,
        &reason,
        &actor,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Debug, Deserialize)]
pub(crate) struct RemoveDependencyQuery {
    reason: Option<String>,
}
