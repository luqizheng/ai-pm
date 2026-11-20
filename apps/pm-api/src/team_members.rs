//! `/v1/pm/teams/{team_id}/members`（P4-ORG Phase 2，原 tenant_members 按
//! Team 维度重写）：Team 成员与角色管理。
//!
//! 授权事实源是 `core.team_member.role`（owner|member 两值），不是 token
//! `roles`（ADR-0001 D5）。撤权即时生效：`AuthedPrincipal.is_admin` 每请求
//! 查库得出（= Team Owner），降级/删除成员后旧 JWT 的下一次请求即被拒。
//!
//! 规则（DR-ORG-02）：仅 Team Owner 可管理成员；任命 Owner / 降级为 Member
//! 走同一 PATCH role 端点；最后一位 Owner 不可退出/降级/移除；Member 可自助
//! 退出（leave）；owner-transfer 已取消（多 Owner 任命制替代）。
//!
//! 所有写操作走同一个 [`run_change`]：单事务、先锁 `core.team` 行串行化本
//! Team 的成员变更、再校验调用者权限与目标归属、最后写
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

use pm_domain::TeamRole;

use pm_application::repo::team_members::{
    ChangeSnapshot, MemberAuditActor, MemberAuditLine, MemberRepo, MemberRepoError, MemberWrite,
};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::repos::member_repo;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/teams/{team_id}/members",
            get(list_handler).post(add_handler),
        )
        .route(
            "/v1/pm/teams/{team_id}/members/{principal_id}",
            get(detail_handler).delete(remove_handler),
        )
        .route(
            "/v1/pm/teams/{team_id}/members/{principal_id}/role",
            patch(set_role_handler),
        )
        .route("/v1/pm/teams/{team_id}/leave", post(leave_handler))
}

// ─────────────────────────────────────────────────────────────── 读端点

async fn list_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(team_id): Path<Uuid>,
) -> Response {
    if let Err(resp) =
        crate::team_llm_guard::verify_active_team_member(&state.pool, &caller, team_id).await
    {
        return resp;
    }
    // 名册只读对所有 ACTIVE 成员开放（基线 §3：Member 可读名册；管理动作
    // 仍走下方各写端点的 Owner 闸）。
    match member_repo(&state.pool).list_members(team_id).await {
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
    Path((team_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) =
        crate::team_llm_guard::verify_active_team_member(&state.pool, &caller, team_id).await
    {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("team owner required");
    }
    match member_repo(&state.pool).find_member(team_id, principal_id).await {
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
    role: String,
}

async fn add_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(team_id): Path<Uuid>,
    Json(body): Json<AddBody>,
) -> Response {
    if let Err(resp) = crate::team_llm_guard::guard_team(&caller, &state.pool, team_id).await {
        return resp;
    }
    run_change(
        &state.pool,
        &caller,
        team_id,
        MemberOp::Add {
            principal_id: body.principal_id,
            role: body.role,
        },
    )
    .await
}

#[derive(Debug, Deserialize)]
struct RoleBody {
    role: String,
}

async fn set_role_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((team_id, principal_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<RoleBody>,
) -> Response {
    if let Err(resp) = crate::team_llm_guard::guard_team(&caller, &state.pool, team_id).await {
        return resp;
    }
    run_change(
        &state.pool,
        &caller,
        team_id,
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
    Path((team_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = crate::team_llm_guard::guard_team(&caller, &state.pool, team_id).await {
        return resp;
    }
    run_change(
        &state.pool,
        &caller,
        team_id,
        MemberOp::Remove { principal_id },
    )
    .await
}

/// 自助退出 Team（基线 §3.2：Owner 可退出但必须剩余至少一位 Owner；最后
/// 一位 Owner 不得退出）。Member 亦可退出。
async fn leave_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(team_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = crate::team_llm_guard::guard_team(&caller, &state.pool, team_id).await {
        return resp;
    }
    run_change(
        &state.pool,
        &caller,
        team_id,
        MemberOp::Leave {
            principal_id: caller.principal_id,
        },
    )
    .await
}

// ─────────────────────────────────────────────────────── 事务与规则

#[derive(Debug, Clone)]
enum MemberOp {
    Add {
        principal_id: Uuid,
        role: String,
    },
    SetRole {
        principal_id: Uuid,
        role: String,
    },
    Remove {
        principal_id: Uuid,
    },
    /// 自助退出：target = caller，不需要 Owner 闸。
    Leave {
        principal_id: Uuid,
    },
}

impl MemberOp {
    fn target(&self) -> Uuid {
        match self {
            Self::Add { principal_id, .. }
            | Self::SetRole { principal_id, .. }
            | Self::Remove { principal_id }
            | Self::Leave { principal_id } => *principal_id,
        }
    }
}

async fn run_change(
    pool: &PgPool,
    caller: &AuthedPrincipal,
    team_id: Uuid,
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
    // authorize 产出的响应体经 Mutex 越过同步回调边界（规则在锁内裁决）。
    let outcome_cell = std::sync::Mutex::new(None::<Value>);
    // 事务管线（锁序/级联/审计）在仓储；authorize 纯函数在锁内回调裁决。
    let result = member_repo(pool)
        .apply_change(
            team_id,
            &actor,
            op.target(),
            &|snap: ChangeSnapshot| {
                let (outcome, writes, audits) = authorize(snap, &op, caller.principal_id)?;
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
            // 规则拒绝：错误码契约（权限类 403，缺失 404，角色值非法 400，其余 409）。
            match code.as_str() {
                "TEAM_OWNER_REQUIRED" | "OWNER_REMOVE_OWNER" => err_forbidden(&message),
                "MEMBER_NOT_FOUND" => err_not_found(&message),
                "ROLE_MUST_BE_MEMBER" => bad_request(&message),
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

/// 纯规则判定：能力矩阵 + 最后 owner 保护（锁内回调，Err 经
/// MemberRepoError::RuleRejected 回滚后由 run_change 映射 409/403/404）。
fn authorize(snap: ChangeSnapshot, op: &MemberOp, actor_id: Uuid) -> DecisionResult {
    let parse = |r: Option<&'static str>| r.and_then(TeamRole::parse);
    let actor_role = parse(snap.actor_role).unwrap_or_else(TeamRole::min_privilege);
    let target_role = parse(snap.target_role);

    match op {
        MemberOp::Add { principal_id, role } => {
            // 仅 Team Owner 可直接添加成员（主路径是邀请）。
            if !actor_role.is_owner() {
                return Err((
                    "TEAM_OWNER_REQUIRED".to_string(),
                    "team owner required".to_string(),
                ));
            }
            if role != "member" {
                // Owner 只能经任命产生（PATCH role），不可直接添加。
                return Err((
                    "ROLE_MUST_BE_MEMBER".to_string(),
                    "team role must be member".to_string(),
                ));
            }
            if target_role.is_some() {
                return Err((
                    "ALREADY_MEMBER".to_string(),
                    "principal is already a member".to_string(),
                ));
            }
            Ok((
                json!({ "principal_id": principal_id, "role": "member" }),
                vec![MemberWrite::Add {
                    principal_id: *principal_id,
                    role: "member",
                    granted_by: actor_id,
                }],
                vec![MemberAuditLine {
                    action: "team_member.added",
                    target: *principal_id,
                    before_role: None,
                    after_role: Some("member"),
                    after_extra: None,
                }],
            ))
        }

        MemberOp::SetRole { principal_id, role } => {
            let Some(role) = TeamRole::parse(&role) else {
                return Err((
                    "ROLE_MUST_BE_MEMBER".to_string(),
                    "team role must be owner or member".to_string(),
                ));
            };
            if !actor_role.is_owner() {
                return Err((
                    "TEAM_OWNER_REQUIRED".to_string(),
                    "team owner required".to_string(),
                ));
            }
            let current = target_role.ok_or_else(|| {
                ("MEMBER_NOT_FOUND".to_string(), "member not found".to_string())
            })?;
            // 任命（member→owner）/ 降级（owner→member）都允许作用于任何成员；
            // 对自己：升 owner 无意义（已是 owner），降级 = 放弃 Owner（多 Owner
            // 且非末位时允许，等价于退出 Owner 身份）。
            if *principal_id == actor_id && role == TeamRole::Owner {
                return Err((
                    "ALREADY_OWNER".to_string(),
                    "caller is already the owner".to_string(),
                ));
            }
            if current == TeamRole::Owner && role == TeamRole::Member && snap.owner_count <= 1 {
                return Err((
                    "OWNER_LAST".to_string(),
                    "the last owner cannot be demoted".to_string(),
                ));
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
                    principal_id: *principal_id,
                    role: role.as_str(),
                }],
                vec![MemberAuditLine {
                    action: "team_member.role_changed",
                    target: *principal_id,
                    before_role: Some(current.as_str()),
                    after_role: Some(role.as_str()),
                    after_extra: None,
                }],
            ))
        }

        MemberOp::Remove { principal_id } => {
            if !actor_role.is_owner() {
                return Err((
                    "TEAM_OWNER_REQUIRED".to_string(),
                    "team owner required".to_string(),
                ));
            }
            let current = target_role.ok_or_else(|| {
                ("MEMBER_NOT_FOUND".to_string(), "member not found".to_string())
            })?;
            if current == TeamRole::Owner && snap.owner_count <= 1 {
                return Err((
                    "OWNER_LAST".to_string(),
                    "the last owner cannot be removed".to_string(),
                ));
            }
            Ok((
                json!({ "principal_id": principal_id, "removed": true }),
                vec![MemberWrite::Remove { principal_id: *principal_id }],
                vec![MemberAuditLine {
                    action: "team_member.removed",
                    target: *principal_id,
                    before_role: Some(current.as_str()),
                    after_role: None,
                    after_extra: None,
                }],
            ))
        }

        MemberOp::Leave { principal_id } => {
            let current = target_role.ok_or_else(|| {
                ("MEMBER_NOT_FOUND".to_string(), "member not found".to_string())
            })?;
            if *principal_id != actor_id {
                return Err((
                    "TEAM_OWNER_REQUIRED".to_string(),
                    "team owner required".to_string(),
                ));
            }
            // 最后一位 Owner 不得退出（基线 §3.2）。
            if current == TeamRole::Owner && snap.owner_count <= 1 {
                return Err((
                    "OWNER_LAST".to_string(),
                    "the last owner cannot leave".to_string(),
                ));
            }
            Ok((
                json!({ "principal_id": principal_id, "left": true }),
                vec![MemberWrite::Remove { principal_id: *principal_id }],
                vec![MemberAuditLine {
                    action: "team_member.left",
                    target: *principal_id,
                    before_role: Some(current.as_str()),
                    after_role: None,
                    after_extra: None,
                }],
            ))
        }
    }
}

use crate::team_llm_guard::{err_forbidden, err_not_found};

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "message": message })),
    )
        .into_response()
}

fn conflict(code: &str, message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({ "code": code, "message": message })),
    )
        .into_response()
}

fn db_error(err: sqlx::Error) -> Response {
    tracing::error!(error = %err, "team member operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "message": "internal error" })),
    )
        .into_response()
}
