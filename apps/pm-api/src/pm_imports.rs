//! `/v1/pm/projects/{id}/imports`：冷启动导入的启动、轮询、取消与 apply。
//!
//! apply 只生成提案（`outcome='proposed'`），不直写 `delivery.*`；写入由既有
//! 审批链的 `decide` 执行。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::project_import::{
    cancel_import, get_import, import_proposals, list_imports, propose_import, start_import,
    terminate_import, ApplyRequest, ImportError, StartImportRequest,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/imports",
            get(list_handler).post(start_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/imports/{import_id}",
            get(get_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/imports/{import_id}/cancel",
            post(cancel_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/imports/{import_id}/terminate",
            post(terminate_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/imports/{import_id}/apply",
            post(apply_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/imports/{import_id}/proposals",
            get(proposals_handler),
        )
}

fn import_error(err: ImportError) -> Response {
    let (status, body) = match err {
        ImportError::ProjectNotFound => (
            StatusCode::NOT_FOUND,
            json!({ "error": "project_not_found" }),
        ),
        ImportError::NoVerifiedRepository => (
            StatusCode::CONFLICT,
            json!({ "error": "no_verified_repository" }),
        ),
        ImportError::ProjectNotEmpty => (
            StatusCode::CONFLICT,
            json!({ "error": "project_not_empty" }),
        ),
        ImportError::ImportInProgress => (
            StatusCode::CONFLICT,
            json!({ "error": "import_in_progress" }),
        ),
        ImportError::RepositoryIdsRequired => (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "repository_ids_required" }),
        ),
        ImportError::RepositoryNotInProject => (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "repository_not_in_project" }),
        ),
        ImportError::AlreadyApplied => {
            (StatusCode::CONFLICT, json!({ "error": "already_applied" }))
        }
        ImportError::NotFound => (
            StatusCode::NOT_FOUND,
            json!({ "error": "import_not_found" }),
        ),
        ImportError::NotCancellable => {
            (StatusCode::CONFLICT, json!({ "error": "not_cancellable" }))
        }
        ImportError::StaleDraft => (StatusCode::CONFLICT, json!({ "error": "stale_draft" })),
        ImportError::NoSelection => (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "no_selection" }),
        ),
        ImportError::ProjectCodeTaken => (
            StatusCode::CONFLICT,
            json!({ "error": "project_code_taken" }),
        ),
        ImportError::NoGroundedOutput => (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "no_grounded_output" }),
        ),
        ImportError::InvalidState(detail) => (
            StatusCode::CONFLICT,
            json!({ "error": "invalid_state", "detail": detail }),
        ),
        ImportError::Model(detail) => {
            tracing::error!(detail = %detail, "import model error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "model_failed" }),
            )
        }
        ImportError::Git(detail) => {
            tracing::error!(detail = %detail, "import git error");
            (StatusCode::BAD_GATEWAY, json!({ "error": "git_failed" }))
        }
        ImportError::Db(err) => {
            tracing::error!(error = %err, "import endpoint database error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "internal" }),
            )
        }
    };
    (status, Json(body)).into_response()
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
        Ok(false) => Err(import_error(ImportError::ProjectNotFound)),
        Err(err) => Err(import_error(ImportError::Db(err))),
    }
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    match list_imports(&state.pool, project_id).await {
        Ok(items) => {
            let total = items.len();
            (
                StatusCode::OK,
                Json(json!({ "items": items, "total": total })),
            )
                .into_response()
        }
        Err(err) => import_error(err),
    }
}

/// 启动导入的请求体。
///
/// N-3：`repository_ids` **必填且非空**——服务端不再替调用方挑「全部 verified
/// 仓库」，导入集合由调用方明确指定（导入创建时固化）。
#[derive(Debug, Deserialize)]
struct StartBody {
    #[serde(default)]
    repository_ids: Vec<Uuid>,
    #[serde(default)]
    idempotency_key: Option<String>,
}

async fn start_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    body: Option<Json<StartBody>>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    let Some(Json(body)) = body else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": "repository_ids_required" })),
        )
            .into_response();
    };
    if body.repository_ids.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": "repository_ids_required" })),
        )
            .into_response();
    }
    let actor = principal.audit_actor();
    let req = StartImportRequest {
        repository_ids: body.repository_ids,
        idempotency_key: body.idempotency_key,
    };
    match start_import(&state.pool, project_id, &actor, &req).await {
        Ok(outcome) => (StatusCode::ACCEPTED, Json(outcome)).into_response(),
        Err(err) => import_error(err),
    }
}

async fn get_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, import_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    match get_import(&state.pool, project_id, import_id).await {
        Ok(detail) => (StatusCode::OK, Json(detail)).into_response(),
        Err(err) => import_error(err),
    }
}

/// 取消导入的请求体（`reason` 可选，用于审计追溯）。
#[derive(Debug, Deserialize)]
struct CancelBody {
    #[serde(default)]
    reason: Option<String>,
}

async fn cancel_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, import_id)): Path<(Uuid, Uuid)>,
    body: Option<Json<CancelBody>>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    let reason = body.and_then(|Json(body)| body.reason);
    let actor = principal.audit_actor();
    match cancel_import(
        &state.pool,
        project_id,
        import_id,
        &actor,
        reason.as_deref(),
    )
    .await
    {
        Ok(()) => (StatusCode::OK, Json(json!({ "status": "cancelled" }))).into_response(),
        Err(err) => import_error(err),
    }
}

/// 显式终止 `proposed` 态的导入（C-2）：提案挂起时也能释放名额。
async fn terminate_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, import_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    let actor = principal.audit_actor();
    match terminate_import(&state.pool, project_id, import_id, &actor).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "status": "cancelled", "terminated": true })),
        )
            .into_response(),
        Err(err) => import_error(err),
    }
}

async fn proposals_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, import_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    match import_proposals(&state.pool, project_id, import_id).await {
        Ok(proposed) => (StatusCode::OK, Json(proposed)).into_response(),
        Err(err) => import_error(err),
    }
}

async fn apply_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, import_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<ApplyRequest>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    match propose_import(&state.pool, project_id, import_id, req).await {
        Ok(proposed) => (StatusCode::OK, Json(proposed)).into_response(),
        Err(ImportError::AlreadyApplied) => (
            StatusCode::OK,
            Json(json!({ "outcome": "already_applied" })),
        )
            .into_response(),
        Err(err) => import_error(err),
    }
}
