//! `/v1/pm/tenants/{tenant_id}/members`（T-021b）：租户成员与角色管理。
//!
//! 授权事实源是 `core.tenant_member.role`，不是 token `roles`（ADR-0001 D5）。
//! 撤权即时生效：`AuthedPrincipal.is_admin` 每请求查库得出，降级/删除成员后
//! 旧 JWT 的下一次请求即被拒，不等 token 过期。
//!
//! 角色值域只有 `owner` / `admin` / `member`——项目工作角色（developer /
//! tester / reviewer / ai_pm）只在 `core.project_member.role`。
//!
//! 所有写操作走同一个 [`apply_member_change`]：单事务、先锁 `core.tenant` 行
//! 串行化本租户的成员变更、再校验调用者权限与目标归属、最后写
//! `governance.tenant_audit_log`。审计失败即整体回滚。

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use pm_domain::TenantRole;

use pm_application::repo::tenant_members::{
    MemberAuditActor, MemberAuditLine, MemberRepo, MemberRepoError, MemberWrite,
};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::member_repo;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/tenants/{tenant_id}/members",
            get(list_handler).post(add_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/members/{principal_id}",
            get(detail_handler).delete(remove_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/members/{principal_id}/role",
            patch(set_role_handler),
        )
        .route(
            "/v1/pm/tenants/{tenant_id}/owner-transfer",
            post(owner_transfer_handler),
        )
}

// ─────────────────────────────────────────────────────────────── 读端点

async fn list_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
) -> Response {
    if let Err(resp) =
        crate::team_llm_guard::verify_active_member(&state.pool, &caller, tenant_id).await
    {
        return resp;
    }
    // 名册只读对所有 ACTIVE 成员开放（0044 Team Onboarding 口径：查看团队
    // 是三种角色的共同权限；管理动作仍走下方各写端点的 admin 闸）。
    match member_repo(&state.pool).list_members(tenant_id).await {
        Ok(rows) => {
            let items: Vec<Value> = rows
                .into_iter()
                .map(|r| {
                    json!({
                        "principal_id": r.principal_id,
                        "display_name": r.display_name,
                        "role": r.role,
                        "is_active": r.is_active,
                        "created_at": r.created_at,
                    })
                })
                .collect();
            Json(json!({ "items": items })).into_response()
        }
        Err(err) => db_error_repo(err),
    }
}

async fn detail_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) =
        crate::team_llm_guard::verify_active_member(&state.pool, &caller, tenant_id).await
    {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    match member_repo(&state.pool).find_member(tenant_id, principal_id).await {
        Ok(Some(r)) => Json(json!({
            "principal_id": r.principal_id,
            "display_name": r.display_name,
            "role": r.role,
            "is_active": r.is_active,
            "created_at": r.created_at,
        }))
        .into_response(),
        Ok(None) => err_not_found("member not found"),
        Err(err) => db_error_repo(err),
    }
    .into_response()
}

// ─────────────────────────────────────────────────────────────── 写端点

#[derive(Debug, Deserialize)]
struct AddBody {
    principal_id: Uuid,
    role: TenantRole,
}

async fn add_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    Json(body): Json<AddBody>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    run_change(
        &state.pool,
        &caller,
        tenant_id,
        MemberOp::Add {
            principal_id: body.principal_id,
            role: body.role,
        },
    )
    .await
}

#[derive(Debug, Deserialize)]
struct RoleBody {
    role: TenantRole,
}

async fn set_role_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, principal_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<RoleBody>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    run_change(
        &state.pool,
        &caller,
        tenant_id,
        MemberOp::SetRole {
            principal_id,
            role: body.role,
        },
    )
    .await
}

async fn remove_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    run_change(
        &state.pool,
        &caller,
        tenant_id,
        MemberOp::Remove { principal_id },
    )
    .await
}

#[derive(Debug, Deserialize)]
struct TransferBody {
    new_owner_principal_id: Uuid,
}

async fn owner_transfer_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(tenant_id): Path<Uuid>,
    Json(body): Json<TransferBody>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    run_change(
        &state.pool,
        &caller,
        tenant_id,
        MemberOp::TransferOwner {
            new_owner: body.new_owner_principal_id,
        },
    )
    .await
}

// ─────────────────────────────────────────────────────── 事务与规则

#[derive(Debug, Clone, Copy)]
enum MemberOp {
    Add {
        principal_id: Uuid,
        role: TenantRole,
    },
    SetRole {
        principal_id: Uuid,
        role: TenantRole,
    },
    Remove {
        principal_id: Uuid,
    },
    TransferOwner {
        new_owner: Uuid,
    },
}

impl MemberOp {
    fn target(self) -> Uuid {
        match self {
            Self::Add { principal_id, .. }
            | Self::SetRole { principal_id, .. }
            | Self::Remove { principal_id } => principal_id,
            Self::TransferOwner { new_owner } => new_owner,
        }
    }
}

async fn run_change(
    pool: &PgPool,
    caller: &AuthedPrincipal,
    tenant_id: Uuid,
    op: MemberOp,
) -> Response {
    // 审计身份四列（C174）。
    let audit_actor = caller.audit_actor();
    let actor = MemberAuditActor {
        principal_id: audit_actor.principal_id,
        identity_id: audit_actor.identity_id,
        client: audit_actor.client,
        auth_mode: audit_actor.auth_mode.unwrap_or_default(),
    };
    // authorize 产出的响应体经 RefCell 越过同步回调边界（规则在锁内裁决）。
    let outcome_cell = std::sync::Mutex::new(None::<Value>);
    // 事务管线（锁序/级联/审计）在仓储；authorize 纯函数在锁内回调裁决。
    let result = member_repo(pool)
        .apply_change(
            tenant_id,
            &actor,
            op.target(),
            &|snap: pm_application::repo::tenant_members::ChangeSnapshot| {
                let (outcome, writes, audits) = authorize(snap, op, caller.principal_id)?;
                *outcome_cell.lock().unwrap() = Some(outcome);
                Ok((writes, audits))
            },
        )
        .await;
    match result {
        Ok(_) => {
            let outcome = outcome_cell
                .into_inner()
                .expect("authorize must set the outcome on success");
            Json(outcome).into_response()
        }
        Err(MemberRepoError::RuleRejected(code, message)) => {
            // 规则拒绝：错误码契约与旧实现一致（权限类 403，缺失 404，其余 409）。
            match code.as_str() {
                "TENANT_ADMIN_REQUIRED" | "ADMIN_GRANT_ADMIN" | "ADMIN_CHANGE_OWNER"
                | "ADMIN_CHANGE_SELF" | "OWNER_REMOVE_OWNER" | "OWNER_TRANSFER_REQUIRED" => {
                    err_forbidden(&message)
                }
                "MEMBER_NOT_FOUND" => err_not_found(&message),
                _ => conflict(&code, &message),
            }
        }
        Err(MemberRepoError::ProjectOwnerDependency) => conflict(
            "PROJECT_OWNER_DEPENDENCY",
            "principal owns at least one project; transfer project ownership first",
        ),
        Err(MemberRepoError::NotFound(m)) => err_not_found(m),
        Err(MemberRepoError::Db(err)) => db_error(err),
    }
}

/// 读端点的仓储错误形态（tracing + 映射）。
fn db_error_repo(err: MemberRepoError) -> Response {
    match err {
        MemberRepoError::Db(e) => db_error(e),
        MemberRepoError::NotFound(m) => err_not_found(m),
        MemberRepoError::RuleRejected(code, m) => conflict(&code, &m),
        MemberRepoError::ProjectOwnerDependency => conflict(
            "PROJECT_OWNER_DEPENDENCY",
            "principal owns at least one project; transfer project ownership first",
        ),
    }
}

/// authorize 的裁决产物：响应体 + DML + 审计行；Err = (错误码, 消息)。
type DecisionResult =
    Result<(Value, Vec<MemberWrite>, Vec<MemberAuditLine>), (String, String)>;

/// 纯规则判定：能力矩阵 + 最后 owner 保护 + owner 转让独占（锁内回调，
/// Err 经 MemberRepoError::RuleRejected 回滚后由 run_change 映射 409/403/404）。
fn authorize(
    snap: pm_application::repo::tenant_members::ChangeSnapshot,
    op: MemberOp,
    actor_id: Uuid,
) -> DecisionResult {
    use pm_domain::TenantRole;
    let parse = |r: Option<&'static str>| r.and_then(TenantRole::parse);
    let actor_role = parse(snap.actor_role).unwrap_or_else(TenantRole::min_privilege);
    if !actor_role.is_tenant_admin() {
        return Err(("TENANT_ADMIN_REQUIRED".to_string(), "tenant admin required".to_string()));
    }
    let target_role = parse(snap.target_role);

    match op {
        MemberOp::Add { principal_id, role } => {
            if role == TenantRole::Owner {
                return Err(("USE_OWNER_TRANSFER".to_string(), "use POST .../owner-transfer".to_string()));
            }
            if role == TenantRole::Admin && actor_role != TenantRole::Owner {
                return Err(("ADMIN_GRANT_ADMIN".to_string(), "admin cannot grant admin".to_string()));
            }
            if target_role.is_some() {
                return Err(("ALREADY_MEMBER".to_string(), "principal is already a member".to_string()));
            }
            Ok((
                json!({ "principal_id": principal_id, "role": role.as_str() }),
                vec![MemberWrite::Add {
                    principal_id,
                    role: role.as_str(),
                    granted_by: actor_id,
                }],
                vec![MemberAuditLine {
                    action: "tenant_member.added",
                    target: principal_id,
                    before_role: None,
                    after_role: Some(role.as_str()),
                    after_extra: None,
                }],
            ))
        }

        MemberOp::SetRole { principal_id, role } => {
            if role == TenantRole::Owner {
                return Err(("USE_OWNER_TRANSFER".to_string(), "use POST .../owner-transfer".to_string()));
            }
            let current = target_role.ok_or_else(|| ("MEMBER_NOT_FOUND".to_string(), "member not found".to_string()))?;

            if actor_role == TenantRole::Admin {
                // admin 只能往降（含把另一个 admin 降为 member），不能提权、
                // 不能动 owner、不能改自己。
                if current == TenantRole::Owner {
                    return Err(("ADMIN_CHANGE_OWNER".to_string(), "admin cannot change an owner".to_string()));
                }
                if role == TenantRole::Admin {
                    return Err(("ADMIN_GRANT_ADMIN".to_string(), "admin cannot grant admin".to_string()));
                }
                if principal_id == actor_id {
                    return Err(("ADMIN_CHANGE_SELF".to_string(), "admin cannot change own role".to_string()));
                }
            }

            if current == TenantRole::Owner && role != TenantRole::Owner && snap.owner_count <= 1 {
                return Err(("OWNER_LAST".to_string(), "the last owner cannot be demoted".to_string()));
            }
            if current == role {
                return Ok((
                    json!({ "principal_id": principal_id, "role": role.as_str() }),
                    vec![],
                    vec![],
                ));
            }
            Ok((
                json!({ "principal_id": principal_id, "role": role.as_str() }),
                vec![MemberWrite::SetRole {
                    principal_id,
                    role: role.as_str(),
                }],
                vec![MemberAuditLine {
                    action: "tenant_member.role_changed",
                    target: principal_id,
                    before_role: Some(current.as_str()),
                    after_role: Some(role.as_str()),
                    after_extra: None,
                }],
            ))
        }

        MemberOp::Remove { principal_id } => {
            let current = target_role.ok_or_else(|| ("MEMBER_NOT_FOUND".to_string(), "member not found".to_string()))?;
            if current == TenantRole::Owner {
                if snap.owner_count <= 1 {
                    return Err(("OWNER_LAST".to_string(), "the last owner cannot be removed".to_string()));
                }
                if actor_role != TenantRole::Owner {
                    return Err(("OWNER_REMOVE_OWNER".to_string(), "only an owner can remove an owner".to_string()));
                }
            }
            Ok((
                json!({ "principal_id": principal_id, "removed": true }),
                vec![MemberWrite::Remove { principal_id }],
                vec![MemberAuditLine {
                    action: "tenant_member.removed",
                    target: principal_id,
                    before_role: Some(current.as_str()),
                    after_role: None,
                    after_extra: None,
                }],
            ))
        }

        MemberOp::TransferOwner { new_owner } => {
            if !actor_role.can_transfer_ownership() {
                return Err(("OWNER_TRANSFER_REQUIRED".to_string(), "only an owner can transfer ownership".to_string()));
            }
            let new_role = target_role.ok_or_else(|| ("MEMBER_NOT_FOUND".to_string(), "member not found".to_string()))?;
            if new_owner == actor_id {
                return Err(("ALREADY_OWNER".to_string(), "caller is already the owner".to_string()));
            }
            if new_role == TenantRole::Owner {
                return Err(("ALREADY_OWNER".to_string(), "target is already an owner".to_string()));
            }
            Ok((
                json!({
                    "previous_owner": actor_id,
                    "new_owner": new_owner,
                }),
                vec![
                    MemberWrite::SetRole {
                        principal_id: actor_id,
                        role: "admin",
                    },
                    MemberWrite::SetRole {
                        principal_id: new_owner,
                        role: "owner",
                    },
                ],
                vec![
                    MemberAuditLine {
                        action: "tenant_member.owner_transferred",
                        target: actor_id,
                        before_role: Some(TenantRole::Owner.as_str()),
                        after_role: Some(TenantRole::Admin.as_str()),
                        after_extra: None,
                    },
                    MemberAuditLine {
                        action: "tenant_member.owner_transferred",
                        target: new_owner,
                        before_role: Some(new_role.as_str()),
                        after_role: Some(TenantRole::Owner.as_str()),
                        after_extra: None,
                    },
                ],
            ))
        }
    }
}

use crate::team_llm_guard::{err_forbidden, err_not_found, guard_tenant};

fn conflict(code: &str, message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({ "code": code, "message": message })),
    )
        .into_response()
}

fn db_error(err: sqlx::Error) -> Response {
    tracing::error!(error = %err, "tenant member operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "message": "internal error" })),
    )
        .into_response()
}
