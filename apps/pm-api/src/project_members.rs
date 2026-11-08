//! `/v1/pm/projects/{project_id}/members`（S01-W13）：Project 成员授权。
//!
//! 授权事实源是 `core.project_member`，不是 token 里的任何 claim。
//! 撤权即时生效：成员行删除后**下一次请求即被拒**（`require_project_member`
//! 每次请求实时查库），不等 token 过期——与租户成员同一口径。
//!
//! 规则（`docs/ai-pm-s01-project-member-authorization-v1.md` §2）：
//!
//! - 真人角色只有 `owner` / `member`；`owner` 只能通过所有权转移获得。
//! - 授权前提：目标必须同时是本租户 `core.tenant_member` 的一行。
//! - Agent 职能行（`ai_pm` / `architect` / `developer` / `tester`）由 bootstrap
//!   独占维护，本 API 不授予也不撤销。
//! - 跨租户与不可见项目一律 404（不可见即不存在）。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use pm_application::project_member::{
    apply_member_change, caller_can_see_members, list_members, MemberOp, ProjectMemberError,
};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/members",
            get(list_handler).post(grant_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/members/{principal_id}",
            delete(revoke_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/owner-transfer",
            post(transfer_handler),
        )
}

// ─────────────────────────────────────────────────────────────── 读端点

async fn list_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = guard_project(&state, &caller, project_id).await {
        return resp;
    }
    match list_members(&state.pool, caller.tenant_id, project_id).await {
        Ok(rows) => {
            let items: Vec<_> = rows
                .into_iter()
                .map(|row| {
                    json!({
                        "principal_id": row.principal_id,
                        "display_name": row.display_name,
                        "role": row.role,
                        "kind": row.kind,
                        "is_active": row.is_active,
                    })
                })
                .collect();
            Json(json!({ "items": items })).into_response()
        }
        Err(err) => map_error(err),
    }
}

// ─────────────────────────────────────────────────────────────── 写端点

#[derive(Debug, Deserialize)]
struct GrantBody {
    principal_id: Uuid,
}

async fn grant_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<GrantBody>,
) -> Response {
    if let Err(resp) = guard_project(&state, &caller, project_id).await {
        return resp;
    }
    run_change(
        &state,
        &caller,
        project_id,
        MemberOp::Grant {
            principal_id: body.principal_id,
        },
    )
    .await
}

async fn revoke_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = guard_project(&state, &caller, project_id).await {
        return resp;
    }
    run_change(
        &state,
        &caller,
        project_id,
        MemberOp::Revoke { principal_id },
    )
    .await
}

#[derive(Debug, Deserialize)]
struct TransferBody {
    new_owner_principal_id: Uuid,
}

async fn transfer_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<TransferBody>,
) -> Response {
    if let Err(resp) = guard_project(&state, &caller, project_id).await {
        return resp;
    }
    run_change(
        &state,
        &caller,
        project_id,
        MemberOp::TransferOwner {
            new_owner: body.new_owner_principal_id,
        },
    )
    .await
}

async fn run_change(
    state: &AppState,
    caller: &AuthedPrincipal,
    project_id: Uuid,
    op: MemberOp,
) -> Response {
    let actor = caller.audit_actor();
    match apply_member_change(&state.pool, &actor, caller.tenant_id, project_id, op).await {
        Ok(outcome) => {
            let mut body = json!({
                "principal_id": outcome.principal_id,
                "revoked": outcome.revoked,
            });
            if let Some(role) = outcome.role {
                body["role"] = json!(role.as_str());
            }
            if let Some(previous) = outcome.previous_owner {
                body["previous_owner"] = json!(previous);
            }
            Json(body).into_response()
        }
        Err(err) => map_error(err),
    }
}

// ─────────────────────────────────────────────────────── 守卫与错误构造

/// 项目可见性守卫：不可见即 404（不泄露项目是否存在）。
///
/// 口径：项目成员可见；租户 admin 可见（他们是项目成员的管理者）。
async fn guard_project(
    state: &AppState,
    caller: &AuthedPrincipal,
    project_id: Uuid,
) -> Result<(), Response> {
    let visible = caller_can_see_members(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
        caller.is_admin,
    )
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "project member visibility check failed");
        db_error()
    })?;

    if visible {
        Ok(())
    } else {
        Err(not_found("project not found"))
    }
}

fn map_error(err: ProjectMemberError) -> Response {
    match err {
        ProjectMemberError::NotFound(_) => not_found("project not found"),
        ProjectMemberError::Forbidden(_) => forbidden("project owner required"),
        ProjectMemberError::Conflict { code, message } => (
            StatusCode::CONFLICT,
            Json(json!({ "code": code, "message": message })),
        )
            .into_response(),
        ProjectMemberError::Db(_) => {
            tracing::error!(error = %err, "project member operation failed");
            db_error()
        }
    }
}

fn not_found(message: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({ "message": message }))).into_response()
}

fn forbidden(message: &str) -> Response {
    (StatusCode::FORBIDDEN, Json(json!({ "message": message }))).into_response()
}

fn db_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "message": "internal error" })),
    )
        .into_response()
}
