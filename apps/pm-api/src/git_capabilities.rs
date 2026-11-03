//! `/v1/pm/projects/{project_id}/repositories/{repository_id}/capabilities`（I3-B）：
//! 仓库级 Git 能力授权管理。
//!
//! 规则（I3 §3.2/§3.3 + W13 管理口径）：
//! - 管理者 = 项目 owner 或租户 admin（`is_project_manager`）；
//! - 默认最小权限：无授权 = 全拒绝（含 `read`）；
//! - 依赖链是**服务端校验规则**：请求集内每个能力的前提必须同集合内已具备
//!   （push 缺 commit → 422）；
//! - 未识别值一律拒绝；
//! - 撤权即时生效：判定层每次请求实时查库。

use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, put},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use pm_application::git_capability::{GitCapabilityError, list_grants, revoke_grant, set_grant};
use pm_application::project_member::is_project_manager;
use pm_domain::{GitCapability, parse_capabilities, prerequisite_violations};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/v1/pm/projects/{project_id}/repositories/{repository_id}/capabilities/{principal_id}",
        get(list_handler).put(set_handler).delete(revoke_handler),
    )
}

/// 管理守卫：项目 owner 或租户 admin；其余一律 404（不泄露存在性）。
async fn guard_manager(
    state: &AppState,
    caller: &AuthedPrincipal,
    project_id: Uuid,
) -> Result<(), Response> {
    match is_project_manager(&state.pool, caller.tenant_id, project_id, caller.principal_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(not_found()),
        Err(err) => Err(map_error(GitCapabilityError::Db(err.to_string()))),
    }
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, repository_id, _principal_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = guard_manager(&state, &caller, project_id).await {
        return resp;
    }
    // 列表按仓库粒度（principal 段仅作路由占位，忽略）。
    match list_grants(&state.pool, caller.tenant_id, project_id, repository_id).await {
        Ok(rows) => {
            let items: Vec<_> = rows
                .into_iter()
                .map(|grant| {
                    json!({
                        "repository_id": grant.repository_id,
                        "repository_name": grant.repository_name,
                        "principal_id": grant.principal_id,
                        "principal_name": grant.principal_name,
                        "principal_kind": grant.principal_kind,
                        "capabilities": grant.capabilities,
                        "granted_by": grant.granted_by,
                    })
                })
                .collect();
            Json(json!({ "items": items, "total": items.len() })).into_response()
        }
        Err(err) => map_error(err),
    }
}

#[derive(Debug, Deserialize)]
struct SetBody {
    /// 六能力字符串（read/modify/commit/push/create_pr/merge）；未识别值 → 422。
    capabilities: Vec<String>,
}

async fn set_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, repository_id, principal_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<SetBody>,
) -> Response {
    if let Err(resp) = guard_manager(&state, &caller, project_id).await {
        return resp;
    }
    let requested = match parse_capabilities(&body.capabilities) {
        Ok(caps) => caps,
        Err(unknown) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": "unknown_capability",
                    "message": format!("unknown capability `{unknown}`"),
                })),
            )
                .into_response();
        }
    };
    // §3.3：依赖链服务端校验（push 缺 commit → 422）。
    let violations = prerequisite_violations(&requested);
    if let Some((cap, missing)) = violations.first() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "prerequisite_missing",
                "message": format!(
                    "capability `{}` requires `{}` in the same grant set",
                    cap.as_str(),
                    missing.as_str()
                ),
            })),
        )
            .into_response();
    }

    let actor = caller.audit_actor();
    match set_grant(
        &state.pool,
        &actor,
        caller.tenant_id,
        project_id,
        repository_id,
        principal_id,
        requested,
    )
    .await
    {
        Ok(caps) => Json(json!({
            "repository_id": repository_id,
            "principal_id": principal_id,
            "capabilities": caps.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(err) => map_error(err),
    }
}

async fn revoke_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, repository_id, principal_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = guard_manager(&state, &caller, project_id).await {
        return resp;
    }
    let actor = caller.audit_actor();
    match revoke_grant(
        &state.pool,
        &actor,
        caller.tenant_id,
        project_id,
        repository_id,
        principal_id,
    )
    .await
    {
        Ok(()) => Json(json!({ "principal_id": principal_id, "revoked": true })).into_response(),
        Err(err) => map_error(err),
    }
}

fn map_error(err: GitCapabilityError) -> Response {
    match err {
        GitCapabilityError::NotFound(_) => not_found(),
        GitCapabilityError::Forbidden(_) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "project owner required" })),
        )
            .into_response(),
        GitCapabilityError::Conflict(message) => (
            StatusCode::CONFLICT,
            Json(json!({ "message": message })),
        )
            .into_response(),
        GitCapabilityError::Validation(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": "validation_error", "message": message })),
        )
            .into_response(),
        GitCapabilityError::Db(_) => {
            tracing::error!(error = %err, "git capability operation failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "message": "internal error" })),
            )
                .into_response()
        }
    }
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "message": "project not found" })),
    )
        .into_response()
}
