//! `/v1/pm/projects/{id}/repositories`、`/v1/pm/projects/{id}/ssh-keys` 与
//! `/v1/pm/repositories/{id}/*`：仓库关系 CRUD、项目级密钥管理与可达性校验。
//!
//! 错误体沿用 `support` 的 `{message}` 约定：@digua/http 对带 `code` 的错误体
//! 会先走错误码翻译，未知码会把 message 覆盖成码本身；只给 message 才能原样透传。
//! 身份沿用调用方传入的 `AuthedPrincipal`（T-021a）。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::guard_query_repo;
use pm_application::repo::GuardQueryRepo;
use pm_application::repository::{
    analyze_repository, archive_repository, bind_repository_ssh_key, confirm_repository_version,
    create_repository, delete_repository, ensure_ssh_key, import_ssh_key, list_repositories,
    list_ssh_keys, rename_ssh_key, revoke_ssh_key, ssh_key_project_id, update_repository,
    verify_repository, AnalyzeRepositoryRequest, ConfirmVersionRequest, CreateRepositoryRequest,
    RepositoryError, UpdateRepositoryRequest,
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
        // S1（2026-11-13）：单仓库手动增量分析 → 快照 intake（幂等键锚 commit
        // + 触发时刻；HEAD 未前进也允许重触发，区别于 verify 联动）。
        .route(
            "/v1/pm/repositories/{repository_id}/analyze",
            post(analyze_handler),
        )
        // S01-03 §6.2（G-05）：人类确认更新采用基准——不自动更新，不触碰正式基线。
        .route(
            "/v1/pm/repositories/{repository_id}/update-version",
            post(confirm_version_handler),
        )
        .route(
            "/v1/pm/repositories/{repository_id}/archive",
            post(archive_handler),
        )
        // 迁移 0022：仓库级绑定/解绑 SSH key（None = 回退项目默认）。
        .route(
            "/v1/pm/repositories/{repository_id}/ssh-key",
            put(bind_key_handler),
        )
        // list / create（幂等 ensure 默认把）；吊销/删除一律按 key id 走 /ssh-keys/{id}。
        .route(
            "/v1/pm/projects/{project_id}/ssh-keys",
            get(list_keys_handler).post(create_key_handler),
        )
        // 导入外部私钥（粘贴通道；按 label 落位，同名 = 替换该把）。
        .route(
            "/v1/pm/projects/{project_id}/ssh-keys/import",
            post(import_key_handler),
        )
        // 吊销指定 key（迁移 0022 多把模型；被 verified 仓库绑定 → 409 key_in_use）；
        // 重命名（label 即名称；冲突 → 409 label_taken）供独立设置页改名。
        .route(
            "/v1/pm/ssh-keys/{key_id}",
            patch(rename_key_handler).delete(revoke_one_key_handler),
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
        RepositoryError::Archived => (StatusCode::CONFLICT, "repository is archived"),
        RepositoryError::HasHistory => (
            StatusCode::CONFLICT,
            "repository has history and cannot be deleted",
        ),
        // 稳定 `error` 码供前端独立设置页本地化兜底（同 AnalyzeInProgress 约定）。
        RepositoryError::KeyInUse => {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "message": "ssh key is still referenced by a verified repository",
                    "error": "key_in_use",
                })),
            )
                .into_response();
        }
        RepositoryError::LabelTaken => {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "message": "ssh key label already in use",
                    "error": "label_taken",
                })),
            )
                .into_response();
        }
        // S1 手动增量分析：进行中 → 409（与 intake task retry 的 NotRetryable 同族）。
        // 额外带稳定 `error` 码，供前端 S4 详情面板做本地化兜底；`message` 仍是
        // 唯一被 @digua/http 透传的字段（`code` 才参与翻译，故不影响既有约定）。
        RepositoryError::AnalyzeInProgress => {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "message": "repository analysis is already in progress",
                    "error": "analysis_in_progress",
                })),
            )
                .into_response();
        }
        // D-5 门：同仓库存在未关闭分析报告 → 409 + report_id 深链（前端跳
        // /project/knowledge?report={id} 处置后重试）。
        RepositoryError::ReportOpen { report_id } => {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "message": "repository has an open analysis report",
                    "error": "report_open",
                    "report_id": report_id.to_string(),
                })),
            )
                .into_response();
        }
        // S1 手动增量分析：未核验 / 无分析锚点 → 422。
        RepositoryError::AnalyzeNoAnchor => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "message": "repository has no verified analysis anchor",
                    "error": "analysis_no_anchor",
                })),
            )
                .into_response();
        }
        // intake 受理校验拒绝（String 错误体，独立早退以避开 &'static str 生命周期）。
        RepositoryError::AnalyzeIntake(detail) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "message": detail })),
            )
                .into_response();
        }
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
    match guard_query_repo(pool)
        .repository_exists(tenant_id, repository_id)
        .await
        .map(|exists| if exists { Some(Uuid::default()) } else { None })
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
    match list_repositories(&state.pool, project_id).await {
        Ok(items) => {
            let total = items.len();
            (
                StatusCode::OK,
                Json(json!({ "items": items, "total": total })),
            )
                .into_response()
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

/// S01-03 §6.2：确认更新采用基准。前端在 `head_commit_sha != selected_commit`
/// （New commits detected）时展示差异并调本端点；人类确认后 Confirmed 基准前移。
async fn confirm_version_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(repository_id): Path<Uuid>,
    Json(req): Json<ConfirmVersionRequest>,
) -> Response {
    if let Err(resp) = repository_in_tenant(&state.pool, repository_id, principal.tenant_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match confirm_repository_version(&state.pool, repository_id, &actor, req).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
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

/// S1（2026-11-13）：单仓库手动增量分析。
///
/// 请求体可省略；`idempotency_key` 由调用方持有防双击（同键重放零副作用），
/// 缺省时服务端按触发时刻生成新键 —— HEAD 未前进也允许重触发。
/// 202 = 快照 intake 已受理；409 = 已有活跃分析；422 = 未核验 / 无分析锚点。
#[derive(Debug, Default, Deserialize)]
struct AnalyzeBody {
    #[serde(default)]
    idempotency_key: Option<String>,
}

async fn analyze_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(repository_id): Path<Uuid>,
    body: Option<Json<AnalyzeBody>>,
) -> Response {
    if let Err(resp) = repository_in_tenant(&state.pool, repository_id, principal.tenant_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    let req = AnalyzeRepositoryRequest {
        idempotency_key: body.and_then(|Json(body)| body.idempotency_key),
    };
    match analyze_repository(&state.pool, repository_id, &actor, req).await {
        Ok(outcome) => (StatusCode::ACCEPTED, Json(outcome)).into_response(),
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

async fn list_keys_handler(
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
    match list_ssh_keys(&state.pool, project_id).await {
        Ok(items) => {
            let total = items.len();
            (
                StatusCode::OK,
                Json(json!({ "items": items, "total": total })),
            )
                .into_response()
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
    let rotate = body.map(|Json(req)| req.rotate).unwrap_or(false);
    let actor = principal.audit_actor();
    match ensure_ssh_key(&state.pool, project_id, &actor, rotate).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(err) => repository_error(err),
    }
}

#[derive(Debug, Deserialize)]
struct ImportKeyRequest {
    private_key: String,
    /// key 名称（label 即身份：同名 = 替换该把）；缺省 'imported'。
    #[serde(default)]
    label: Option<String>,
}

/// 粘贴导入外部私钥：私钥只进不出，响应仅含推导出的公钥与指纹。
/// 无效私钥（格式错误 / 带口令）→ 400 Validation。
async fn import_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<ImportKeyRequest>,
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
    match import_ssh_key(
        &state.pool,
        project_id,
        &actor,
        &req.private_key,
        req.label.as_deref().unwrap_or("imported"),
    )
    .await
    {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(err) => repository_error(err),
    }
}

#[derive(Debug, Deserialize)]
struct BindKeyRequest {
    /// 要绑定的 key id；null = 解绑（回退项目默认）。
    #[serde(default)]
    ssh_key_id: Option<Uuid>,
}

/// PUT /repositories/{id}/ssh-key：仓库绑定/解绑一把 key（迁移 0022）。
/// 越权 key / 已吊销 key → 400；key 不存在 → 404；归档仓库 → 409。
async fn bind_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(repository_id): Path<Uuid>,
    Json(req): Json<BindKeyRequest>,
) -> Response {
    if let Err(resp) = repository_in_tenant(&state.pool, repository_id, principal.tenant_id).await {
        return resp;
    }
    let actor = principal.audit_actor();
    match bind_repository_ssh_key(&state.pool, repository_id, &actor, req.ssh_key_id).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => repository_error(err),
    }
}

/// DELETE /ssh-keys/{key_id}：吊销指定 key（被 verified 仓库绑定 → 409 key_in_use）。
#[derive(Deserialize)]
struct RenameKeyBody {
    label: String,
}

/// S2（2026-11-14）：重命名一把 key（label 即名称；独立设置页改名通道）。
async fn rename_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(key_id): Path<Uuid>,
    Json(body): Json<RenameKeyBody>,
) -> Response {
    // key 不存在或越权一律按不存在处理（key → project_id → 租户/成员边界）。
    let key_project = match ssh_key_project_id(&state.pool, key_id).await {
        Ok(project_id) => project_id,
        Err(err) => return repository_error(err),
    };
    let Some(project_id) = key_project else {
        return repository_error(RepositoryError::SshKeyNotFound);
    };
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
    match rename_ssh_key(&state.pool, project_id, &actor, key_id, &body.label).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(err) => repository_error(err),
    }
}

async fn revoke_one_key_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(key_id): Path<Uuid>,
) -> Response {
    // key 不存在或越权一律按不存在处理（key → project_id → 租户/成员边界）。
    let key_project = match ssh_key_project_id(&state.pool, key_id).await {
        Ok(project_id) => project_id,
        Err(err) => return repository_error(err),
    };
    let Some(project_id) = key_project else {
        return repository_error(RepositoryError::SshKeyNotFound);
    };
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
    match revoke_ssh_key(&state.pool, project_id, &actor, key_id).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "id": key_id }))).into_response(),
        Err(err) => repository_error(err),
    }
}
