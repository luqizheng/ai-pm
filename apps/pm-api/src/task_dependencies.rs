//! `/v1/pm/tasks/{task_id}/dependencies`：Task 前置依赖的读写端点（T-024）。
//!
//! - GET  列出上游/下游（元素 `{id, title, status}`）；
//! - POST body `{depends_on_task_id}`，为下游增加一条指向前置的边；
//! - DELETE 幂等删除边（不存在也返回 204）。
//!
//! 可见性沿用 T-022：任务不存在、跨租户或调用者非项目成员一律 404，不泄露存在性。
//! 写路径的租户前置校验在这里完成；应用层只看 `project_id`（组合外键已保证同租户）。

use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use pm_application::task_dependency::{
    self, DependencyError, DependencyGraph,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{ApiError, ApiResult, require_project_visible};

impl From<DependencyError> for ApiError {
    fn from(e: DependencyError) -> Self {
        match e {
            DependencyError::TaskNotFound => ApiError::NotFound("task_not_found"),
            DependencyError::SelfDependency => ApiError::Conflict("SELF_DEPENDENCY"),
            DependencyError::TaskNotPending(_) => ApiError::Conflict("TASK_NOT_PENDING"),
            DependencyError::Duplicate => ApiError::Conflict("DUPLICATE_DEPENDENCY"),
            DependencyError::CrossProject => ApiError::Conflict("CROSS_PROJECT_DEPENDENCY"),
            DependencyError::Cycle => ApiError::Conflict("DEPENDENCY_CYCLE"),
            DependencyError::Db(err) => ApiError::Internal(err),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddDependencyBody {
    pub depends_on_task_id: Uuid,
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
    let project_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM delivery.task WHERE id = $1 AND tenant_id = $2",
    )
    .bind(task_id)
    .bind(principal.tenant_id)
    .fetch_optional(&state.pool)
    .await?;
    let project_id = project_id.ok_or(ApiError::NotFound("task_not_found"))?;
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
    let graph = task_dependency::add_dependency(
        &state.pool,
        task_id,
        body.depends_on_task_id,
        &actor,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(graph)).into_response())
}

async fn remove_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((task_id, depends_on_task_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    require_task_project(&state, task_id, &principal).await?;
    let actor = principal.audit_actor();
    task_dependency::remove_dependency(&state.pool, task_id, depends_on_task_id, &actor).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}