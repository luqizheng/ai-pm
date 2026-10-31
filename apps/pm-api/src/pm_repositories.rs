//! `/v1/pm/projects/{id}/repositories` 与 `/v1/pm/projects/{id}/ssh-key`
//! （P0：项目 ↔ 代码仓库关系的纯 CRUD）。
//!
//! 错误体沿用 `support` 的 `{message}` 约定：@digua/http 对带 `code` 的错误体
//! 会先走错误码翻译，未知码会把 message 覆盖成码本身；只给 message 才能原样透传。
//! 身份沿用 `pm_write::owner_actor` 的 V1 约定（iam↔principal 映射表落地后收紧）。

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;
use uuid::Uuid;

use crate::routes::AppState;
use pm_application::repository::{
    CreateRepositoryRequest, RepositoryError, UpdateRepositoryRequest, create_repository,
    delete_repository, delete_ssh_key, generate_ssh_key, get_ssh_key, list_repositories,
    update_repository,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/repositories",
            get(list_handler).post(create_handler),
        )
        .route(
            "/v1/pm/repositories/{repository_id}",
            axum::routing::patch(update_handler).delete(delete_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/ssh-key",
            get(get_key_handler)
                .post(generate_key_handler)
                .delete(delete_key_handler),
        )
}

/// `RepositoryError` → `{message}` 响应；映射与前端错误文案一一对应。
fn repository_error(err: RepositoryError) -> Response {
    let (status, message) = match err {
        RepositoryError::ProjectNotFound => (StatusCode::NOT_FOUND, "project not found"),
        RepositoryError::NotFound => (StatusCode::NOT_FOUND, "repository not found"),
        RepositoryError::SshKeyNotFound => (StatusCode::NOT_FOUND, "ssh key not found"),
        RepositoryError::InvalidUrl => (StatusCode::BAD_REQUEST, "invalid repository url"),
        RepositoryError::Duplicate => (
            StatusCode::CONFLICT,
            "repository url already bound to this project",
        ),
        RepositoryError::RevisionConflict => (StatusCode::CONFLICT, "revision conflict"),
        RepositoryError::InvalidRole => (StatusCode::BAD_REQUEST, "invalid repository role"),
        RepositoryError::Validation(message) => (StatusCode::BAD_REQUEST, message),
        RepositoryError::SecretKeyMissing | RepositoryError::Crypto => (
            StatusCode::SERVICE_UNAVAILABLE,
            "AI_PM_SECRET_KEY is not configured",
        ),
        RepositoryError::SshKeygen(detail) => {
            tracing::error!(detail = %detail, "ssh-keygen failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ssh-keygen is unavailable on the server",
            )
        }
        RepositoryError::Db(err) => {
            tracing::error!(error = %err, "repository endpoint database error");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    };
    (
        status,
        Json(json!({ "message": message })),
    )
        .into_response()
}

async fn list_handler(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Response {
    match list_repositories(&state.pool, project_id).await {
        Ok(items) => {
            let total = items.len();
            (StatusCode::OK, Json(json!({ "items": items, "total": total }))).into_response()
        }
        Err(err) => repository_error(err),
    }
}

async fn create_handler(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateRepositoryRequest>,
) -> Response {
    match create_repository(&state.pool, project_id, None, req).await {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn update_handler(
    State(state): State<AppState>,
    Path(repository_id): Path<Uuid>,
    Json(req): Json<UpdateRepositoryRequest>,
) -> Response {
    match update_repository(&state.pool, repository_id, None, req).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn delete_handler(
    State(state): State<AppState>,
    Path(repository_id): Path<Uuid>,
) -> Response {
    match delete_repository(&state.pool, repository_id, None).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "id": repository_id }))).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn get_key_handler(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Response {
    match get_ssh_key(&state.pool, project_id).await {
        Ok(Some(view)) => (StatusCode::OK, Json(view)).into_response(),
        Ok(None) => repository_error(RepositoryError::SshKeyNotFound),
        Err(err) => repository_error(err),
    }
}

async fn generate_key_handler(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Response {
    match generate_ssh_key(&state.pool, project_id, None).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn delete_key_handler(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Response {
    match delete_ssh_key(&state.pool, project_id, None).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "project_id": project_id }))).into_response(),
        Err(err) => repository_error(err),
    }
}