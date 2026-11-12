//! `/v1/pm/tasks/{task_id}/repositories`：Task ↔ 仓库绑定的读写端点（WP-C）。
//!
//! - GET    列出绑定（含 `repository_verified` 标注——快照只取 verified）；
//! - POST   幂等绑定（多条 entry；冲突时未提供字段保留现值），返回绑定全集；
//! - DELETE `/repositories/{repository_id}` 幂等解绑（不存在也返回 204）。
//!
//! 可见性沿用 T-022/T-024 先例：任务不存在、跨租户或调用者非项目成员一律 404，
//! 不泄露存在性。写路径的租户前置校验在这里完成；应用层只看 `tenant_id + project_id`。
//!
//! 写入口面（约束 6）：MCP 四工具无仓库参数；归属唯一写入路径 = 本端点
//! （应用服务 `task_repository_binding`），执行期对未绑定仓库 deny。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get},
    Json, Router,
};
use pm_application::task_repository_binding::{self, BindRepositoryEntry, TaskRepoBindingError};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use pm_application::repo::GuardQueryRepo;
use crate::support::{require_project_visible, ApiError, ApiResult};

impl From<TaskRepoBindingError> for ApiError {
    fn from(e: TaskRepoBindingError) -> Self {
        match e {
            TaskRepoBindingError::TaskNotFound => ApiError::NotFound("task_not_found"),
            TaskRepoBindingError::RepositoryNotFound => ApiError::NotFound("repository_not_found"),
            TaskRepoBindingError::RepositoryNotInProject => {
                ApiError::Conflict("REPOSITORY_NOT_IN_PROJECT")
            }
            TaskRepoBindingError::RepositoryNotVerified(status) => ApiError::Text(
                StatusCode::CONFLICT,
                format!("REPOSITORY_NOT_VERIFIED:{status}"),
            ),
            TaskRepoBindingError::InvalidRole(msg) => ApiError::Text(StatusCode::BAD_REQUEST, msg),
            TaskRepoBindingError::Validation(msg) => ApiError::Text(StatusCode::BAD_REQUEST, msg),
            TaskRepoBindingError::Db(err) => ApiError::Internal(err),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct BindRepositoriesBody {
    pub entries: Vec<BindEntryBody>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BindEntryBody {
    pub repository_id: Uuid,
    pub write_access: Option<bool>,
    pub base_ref: Option<String>,
    pub role: Option<String>,
    pub path_scope: Option<Vec<String>>,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/tasks/{task_id}/repositories",
            get(list_handler).post(bind_handler),
        )
        .route(
            "/v1/pm/tasks/{task_id}/repositories/{repository_id}",
            delete(unbind_handler),
        )
}

/// 任务的项目边界：不存在 / 跨租户 / 非成员 → 404（T-022/T-024 先例）。
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
) -> ApiResult<Json<Vec<task_repository_binding::TaskRepositoryBinding>>> {
    let project_id = require_task_project(&state, task_id, &principal).await?;
    let bindings = task_repository_binding::list_task_repositories(
        &state.pool,
        principal.tenant_id,
        project_id,
        task_id,
    )
    .await?;
    Ok(Json(bindings))
}

async fn bind_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(task_id): Path<Uuid>,
    Json(body): Json<BindRepositoriesBody>,
) -> ApiResult<Json<Vec<task_repository_binding::TaskRepositoryBinding>>> {
    let project_id = require_task_project(&state, task_id, &principal).await?;
    let actor = principal.audit_actor();
    let entries: Vec<BindRepositoryEntry> = body
        .entries
        .into_iter()
        .map(|b| BindRepositoryEntry {
            repository_id: b.repository_id,
            write_access: b.write_access,
            base_ref: b.base_ref,
            role: b.role,
            path_scope: b.path_scope,
        })
        .collect();
    let bindings = task_repository_binding::bind_task_repositories(
        &state.pool,
        principal.tenant_id,
        project_id,
        task_id,
        &entries,
        &actor,
    )
    .await?;
    Ok(Json(bindings))
}

async fn unbind_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((task_id, repository_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    let project_id = require_task_project(&state, task_id, &principal).await?;
    let actor = principal.audit_actor();
    task_repository_binding::unbind_task_repository(
        &state.pool,
        principal.tenant_id,
        project_id,
        task_id,
        repository_id,
        &actor,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
