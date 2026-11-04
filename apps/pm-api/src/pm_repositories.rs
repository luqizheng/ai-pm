//! `/v1/pm/projects/{id}/repositories`、`/v1/pm/projects/{id}/ssh-keys` 与
//! `/v1/pm/repositories/{id}/*`：仓库关系 CRUD、项目级密钥管理与可达性校验。
//!
//! 错误体沿用 `support` 的 `{message}` 约定：@digua/http 对带 `code` 的错误体
//! 会先走错误码翻译，未知码会把 message 覆盖成码本身；只给 message 才能原样透传。
//! 身份沿用调用方传入的 `AuthedPrincipal`（T-021a）。

use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::repository::{
    CreateRepositoryRequest, RepositoryError, UpdateRepositoryRequest, archive_repository,
    create_repository, delete_repository, delete_ssh_key, ensure_ssh_key, generate_ssh_key,
    get_ssh_key, list_repositories, list_ssh_keys, revoke_ssh_key, update_repository,
    verify_repository,
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
            "/v1/pm/repositories/{repository_id}/verify",
            post(verify_handler),
        )
        .route(
            "/v1/pm/repositories/{repository_id}/archive",
            post(archive_handler),
        )
        // 旧名字（首版前端）：一项目一把，POST 视为重新生成。
        .route(
            "/v1/pm/projects/{project_id}/ssh-key",
            get(get_key_handler)
                .post(generate_key_handler)
                .delete(delete_key_handler),
        )
        // 新契约：list / create（幂等）/ revoke。
        .route(
            "/v1/pm/projects/{project_id}/ssh-keys",
            get(list_keys_handler)
                .post(create_key_handler)
                .delete(revoke_key_handler),
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
        RepositoryError::HasHistory => (
            StatusCode::CONFLICT,
            "repository has history and cannot be deleted",
        ),
        RepositoryError::KeyInUse => (
            StatusCode::CONFLICT,
            "ssh key is still referenced by a verified repository",
        ),
        RepositoryError::Git(detail) => {
            tracing::error!(detail = %detail, "git access failed");
            (StatusCode::BAD_GATEWAY, "git access failed")
        }
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
    (status, Json(json!({ "message": message }))).into_response()
}

/// 租户 + 成员边界（对外入口，T-022）：跨租户或调用者不是项目成员的
/// project 一律按不存在处理。
async fn project_in_tenant(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> Result<(), Response> {
    match crate::support::project_visible(pool, project_id, tenant_id, principal_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(repository_error(RepositoryError::ProjectNotFound)),
        Err(err) => Err(repository_error(RepositoryError::Db(err))),
    }
}

async fn repository_in_tenant(
    pool: &sqlx::PgPool,
    repository_id: Uuid,
    tenant_id: Uuid,
) -> Result<(), Response> {
    match sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM integration.project_repository WHERE id = $1 AND tenant_id = $2",
    )
    .bind(repository_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(repository_error(RepositoryError::NotFound)),
        Err(err) => Err(repository_error(RepositoryError::Db(err))),
    }
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
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
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateRepositoryRequest>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match create_repository(&state.pool, project_id, &actor, req).await {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn update_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(repository_id): Path<Uuid>,
    Json(req): Json<UpdateRepositoryRequest>,
) -> Response {
    if let Err(resp) = repository_in_tenant(&state.pool, repository_id, principal.tenant_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match update_repository(&state.pool, repository_id, &actor, req).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn delete_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(repository_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = repository_in_tenant(&state.pool, repository_id, principal.tenant_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match delete_repository(&state.pool, repository_id, &actor).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "id": repository_id }))).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn verify_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(repository_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = repository_in_tenant(&state.pool, repository_id, principal.tenant_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match verify_repository(&state.pool, repository_id, &actor).await {
        Ok(outcome) => (StatusCode::OK, Json(outcome)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn archive_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(repository_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = repository_in_tenant(&state.pool, repository_id, principal.tenant_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match archive_repository(&state.pool, repository_id, &actor).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn get_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    match get_ssh_key(&state.pool, project_id).await {
        Ok(Some(view)) => (StatusCode::OK, Json(view)).into_response(),
        Ok(None) => repository_error(RepositoryError::SshKeyNotFound),
        Err(err) => repository_error(err),
    }
}

async fn generate_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match generate_ssh_key(&state.pool, project_id, &actor).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn delete_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match delete_ssh_key(&state.pool, project_id, &actor).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "project_id": project_id }))).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn list_keys_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    match list_ssh_keys(&state.pool, project_id).await {
        Ok(items) => {
            let total = items.len();
            (StatusCode::OK, Json(json!({ "items": items, "total": total }))).into_response()
        }
        Err(err) => repository_error(err),
    }
}

#[derive(Debug, Default, Deserialize)]
struct CreateKeyRequest {
    #[serde(default)]
    rotate: bool,
}

async fn create_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    body: Option<Json<CreateKeyRequest>>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let rotate = body.map(|Json(req)| req.rotate).unwrap_or(false);
    let actor = principal.audit_actor();
    match ensure_ssh_key(&state.pool, project_id, &actor, rotate).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn revoke_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match revoke_ssh_key(&state.pool, project_id, &actor).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "project_id": project_id }))).into_response(),
        Err(err) => repository_error(err),
    }
}