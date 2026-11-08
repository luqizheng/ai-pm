//! `/v1/pm/tenants/{tenant_id}/llm-providers*`：Team-owned Provider 管理面
//! （设计稿 `docs/ui/smark-pm-team-llm-provider-management-v1.md` §6/§12/§16；
//! DEC-LLM-001~004；核验报告 R-LLM-01 修复载体）。
//!
//! 权限矩阵（§12）：查看/管理/替换凭证/测试/启停/归档/删除全部限
//! Team Owner/Admin（`is_admin` = `tenant_role.is_tenant_admin()`，每请求查库，
//! 降级即时生效）；Project Admin 与 Member 无入口。跨租户一律 404。
//!
//! 旧全局端点 `/v1/pm/llm/providers*`（0007 全局表、零角色守卫）已随 0041
//! 退役——本模块是唯一 Provider 管理入口。
//!
//! 错误体沿用 `/v1/pm/*` 约定：只带 `{message}`；CAS 冲突 409 带
//! `{code, message}`（`conflict()`，与 tenant_members 同款）。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::team_llm::{
    create_team_provider, delete_team_provider, get_project_ai_policy, list_team_providers,
    replace_team_credential, set_project_ai_policy, set_team_provider_status, test_team_provider,
    update_team_provider, CreateTeamProviderRequest, ReplaceCredentialRequest, SetStatusRequest,
    TeamLlmError, UpdateTeamProviderRequest,
};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::team_llm_guard::{err_forbidden, err_not_found, guard_tenant};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-providers",
            get(list_handler).post(create_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-providers/{provider_id}",
            axum::routing::patch(update_handler).delete(delete_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-providers/{provider_id}/test",
            post(test_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-providers/{provider_id}/credential",
            post(credential_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/llm-providers/{provider_id}/status",
            post(status_handler),
        )
        // 项目 AI 模型策略（§10.2/§16）：项目在团队范围内指定 Provider。
        .route(
            "/v1/pm/projects/{project_id}/ai-model-policies",
            get(project_policy_handler).put(project_policy_put_handler),
        )
}

/// 项目域守卫：项目必须属于调用者租户，且调用者为
/// manager（租户 owner/admin 或项目 owner，§12「Team Owner/Admin ✓、
/// Project Admin ✓」口径）。
async fn guard_project_manager(
    state: &AppState,
    caller: &AuthedPrincipal,
    project_id: Uuid,
) -> Result<(), Response> {
    let project_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_one(&state.pool)
            .await
            .ok();
    if project_tenant.is_none() {
        return Err(err_not_found("project not found"));
    }
    let _ = project_tenant
        .filter(|t| t == &caller.tenant_id)
        .ok_or_else(|| err_not_found("project not found"))?;
    match pm_application::project_member::is_project_manager(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(err_forbidden("project manager required")),
        Err(_) => Err(err_not_found("project not found")),
    }
}

#[derive(serde::Deserialize)]
struct ProjectPolicyPutBody {
    /// None = 恢复继承 Team Default（删除指定行）。
    #[serde(default)]
    provider_id: Option<Uuid>,
    #[serde(default)]
    expected_revision: i64,
}

async fn project_policy_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = guard_project_manager(&state, &caller, project_id).await {
        return resp;
    }
    match get_project_ai_policy(&state.pool, caller.tenant_id, project_id).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}

async fn project_policy_put_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ProjectPolicyPutBody>,
) -> Response {
    if let Err(resp) = guard_project_manager(&state, &caller, project_id).await {
        return resp;
    }
    match set_project_ai_policy(
        &state.pool,
        &audit_actor(&caller),
        caller.tenant_id,
        project_id,
        body.provider_id,
        body.expected_revision,
    )
    .await
    {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}

fn audit_actor(caller: &AuthedPrincipal) -> AuditActor {
    caller.audit_actor()
}

fn error_response(err: TeamLlmError) -> Response {
    let (status, body) = match &err {
        TeamLlmError::NotFound => (
            StatusCode::NOT_FOUND,
            json!({ "message": "provider not found" }),
        ),
        TeamLlmError::Validation(message) => {
            (StatusCode::BAD_REQUEST, json!({ "message": message }))
        }
        TeamLlmError::AdapterUnavailable(message) => (
            StatusCode::BAD_REQUEST,
            json!({ "message": message, "code": "adapter_unavailable" }),
        ),
        TeamLlmError::StatusTransition(message) => (
            StatusCode::CONFLICT,
            json!({ "code": "status_transition", "message": message }),
        ),
        TeamLlmError::ReferencedBy(what) => (
            StatusCode::CONFLICT,
            json!({ "code": "referenced", "message": format!("provider is referenced by {what}") }),
        ),
        TeamLlmError::RevisionConflict { expected, actual } => (
            StatusCode::CONFLICT,
            json!({ "code": "revision_conflict", "message": format!("revision conflict: expected {expected}, actual {actual}"), "actual_revision": actual }),
        ),
        TeamLlmError::SecretKeyMissing => (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "message": "AI_PM_SECRET_KEY is not configured; API key cannot be stored" }),
        ),
        TeamLlmError::Crypto => {
            tracing::error!(error = %err, "team llm api key crypto failure");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "message": "internal error" }),
            )
        }
        TeamLlmError::Db(_) => {
            tracing::error!(error = %err, "team llm provider db failure");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "message": "internal error" }),
            )
        }
    };
    (status, Json(body)).into_response()
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    match list_team_providers(&state.pool, tenant_id).await {
        Ok(items) => {
            let total = items.len();
            (
                StatusCode::OK,
                Json(json!({ "items": items, "total": total })),
            )
                .into_response()
        }
        Err(err) => error_response(TeamLlmError::Db(err)),
    }
}

async fn create_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    Json(body): Json<CreateTeamProviderRequest>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    match create_team_provider(&state.pool, &audit_actor(&caller), tenant_id, body).await {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}

async fn update_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, provider_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateTeamProviderRequest>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    match update_team_provider(
        &state.pool,
        &audit_actor(&caller),
        tenant_id,
        provider_id,
        body,
    )
    .await
    {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}

async fn delete_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, provider_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    match delete_team_provider(&state.pool, &audit_actor(&caller), tenant_id, provider_id).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "deleted": true }))).into_response(),
        Err(err) => error_response(err),
    }
}

async fn test_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, provider_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    match test_team_provider(&state.pool, &audit_actor(&caller), tenant_id, provider_id).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(err) => error_response(err),
    }
}

async fn credential_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, provider_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ReplaceCredentialRequest>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    match replace_team_credential(
        &state.pool,
        &audit_actor(&caller),
        tenant_id,
        provider_id,
        body,
    )
    .await
    {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}

async fn status_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, provider_id)): Path<(Uuid, Uuid)>,
    body: Option<Json<Value>>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    let raw = body.map(|Json(value)| value).unwrap_or(Value::Null);
    let request: SetStatusRequest = match serde_json::from_value(raw) {
        Ok(request) => request,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "message": "action must be enable|disable|archive" })),
            )
                .into_response();
        }
    };
    match set_team_provider_status(
        &state.pool,
        &audit_actor(&caller),
        tenant_id,
        provider_id,
        request,
    )
    .await
    {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(err) => error_response(err),
    }
}
