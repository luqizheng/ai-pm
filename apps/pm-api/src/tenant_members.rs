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
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use pm_domain::TenantRole;

use pm_application::actor::AuditActor;
use pm_application::project_member::revoke_principal_memberships;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;

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
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    // 名册只读对所有 ACTIVE 成员开放（0044 Team Onboarding 口径：查看团队
    // 是三种角色的共同权限；管理动作仍走下方各写端点的 admin 闸）。
    let rows = sqlx::query_as::<_, (Uuid, String, String, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT m.principal_id, p.display_name, m.role, p.is_active, m.created_at \
           FROM core.tenant_member m \
           JOIN core.principal p ON p.id = m.principal_id \
          WHERE m.tenant_id = $1 \
          ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, \
                   m.created_at",
    )
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await;

    match rows {
        Ok(rows) => {
            let items: Vec<Value> = rows
                .into_iter()
                .map(|(id, name, role, is_active, created_at)| {
                    json!({
                        "principal_id": id,
                        "display_name": name,
                        "role": role,
                        "is_active": is_active,
                        "created_at": created_at,
                    })
                })
                .collect();
            Json(json!({ "items": items })).into_response()
        }
        Err(err) => db_error(err),
    }
}

async fn detail_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((tenant_id, principal_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) = guard_tenant(&caller, tenant_id) {
        return resp;
    }
    if !caller.is_admin {
        return err_forbidden("tenant admin required");
    }
    let row = sqlx::query_as::<_, (Uuid, String, String, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT m.principal_id, p.display_name, m.role, p.is_active, m.created_at \
           FROM core.tenant_member m \
           JOIN core.principal p ON p.id = m.principal_id \
          WHERE m.tenant_id = $1 AND m.principal_id = $2",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some((id, name, role, is_active, created_at))) => Json(json!({
            "principal_id": id,
            "display_name": name,
            "role": role,
            "is_active": is_active,
            "created_at": created_at,
        }))
        .into_response(),
        Ok(None) => err_not_found("member not found"),
        Err(err) => db_error(err),
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
    Add { principal_id: Uuid, role: TenantRole },
    SetRole { principal_id: Uuid, role: TenantRole },
    Remove { principal_id: Uuid },
    TransferOwner { new_owner: Uuid },
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
    match apply_member_change(pool, caller, tenant_id, op).await {
        Ok(outcome) => Json(outcome).into_response(),
        Err(err) => err,
    }
}

/// 单事务完成一次成员变更；返回给前端的 JSON 与审计载荷同源。
async fn apply_member_change(
    pool: &PgPool,
    caller: &AuthedPrincipal,
    tenant_id: Uuid,
    op: MemberOp,
) -> Result<Value, Response> {
    let mut tx = pool.begin().await.map_err(db_error)?;

    // 1) 锁租户行：同一租户的所有成员变更串行化。只锁 owner 行不够——两个
    //    事务可以各锁不同的 owner 行，在 READ COMMITTED 下同时读到
    //    「owner 数 = 2」而互相降级，最终剩 0 个 owner。
    let locked: Option<Uuid> = sqlx::query_scalar("SELECT id FROM core.tenant WHERE id = $1 FOR UPDATE")
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
    if locked.is_none() {
        return Err(err_not_found("tenant not found"));
    }

    // 2) 校验调用者当前权限（读库，不读 token）。
    let actor: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member \
          WHERE tenant_id = $1 AND principal_id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(caller.principal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let actor_role = actor
        .as_deref()
        .and_then(TenantRole::parse)
        .unwrap_or_else(TenantRole::min_privilege);
    if !actor_role.is_tenant_admin() {
        return Err(err_forbidden("tenant admin required"));
    }

    // 3) 锁定目标成员行（可能不存在）。
    let target_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member \
          WHERE tenant_id = $1 AND principal_id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(op.target())
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let target_role = target_role.as_deref().and_then(TenantRole::parse);

    let owner_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1 AND role = 'owner'")
            .bind(tenant_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;

    let Plan {
        outcome: plan_outcome,
        mut audits,
    } = authorize(actor_role, op, target_role, owner_count, caller.principal_id)?;
    let seen = actor_record(caller);

    // S01-W13：移除租户成员前，若该人是某项目的 owner，则**拒绝移除**——
    // 否则级联撤权会静默产生「无主项目」（谁也管不了，也转移不了所有权）。
    if let MemberOp::Remove { principal_id } = op {
        let owned: Vec<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM core.project_member \
              WHERE tenant_id = $1 AND principal_id = $2 AND role = 'owner' FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;
        if !owned.is_empty() {
            return Err(conflict(
                "PROJECT_OWNER_DEPENDENCY",
                "principal owns at least one project; transfer project ownership first",
            ));
        }
    }

    let writes = apply_writes(&mut tx, tenant_id, op, caller.principal_id, &seen).await?;

    // 级联撤销的项目成员关系数写进租户审计行的 after_data。
    if writes.project_memberships_revoked > 0 {
        for line in audits.iter_mut() {
            if line.action == "tenant_member.removed" {
                line.after_extra = Some(json!({
                    "project_memberships_revoked": writes.project_memberships_revoked,
                    "revoked_project_ids": writes.revoked_project_ids,
                }));
            }
        }
    }

    for audit in &audits {
        insert_audit(&mut tx, tenant_id, &seen, audit).await?;
    }

    let outcome = plan_outcome;
    tx.commit().await.map_err(db_error)?;
    Ok(outcome)
}

/// `apply_writes` 的副产物：级联撤销的项目成员关系。
#[derive(Debug, Default)]
struct WriteOutcome {
    project_memberships_revoked: usize,
    revoked_project_ids: Vec<Uuid>,
}

/// 落地 `tenant_member` 的 DML。规则校验已由 [`authorize`] 完成，这里只写库。
async fn apply_writes(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    op: MemberOp,
    actor_id: Uuid,
    seen: &ActorRecord,
) -> Result<WriteOutcome, Response> {
    match op {
        MemberOp::Add { principal_id, role } => {
            sqlx::query(
                "INSERT INTO core.tenant_member \
                 (tenant_id, principal_id, role, granted_by) VALUES ($1, $2, $3, $4)",
            )
            .bind(tenant_id)
            .bind(principal_id)
            .bind(role.as_str())
            .bind(actor_id)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
        }
        MemberOp::SetRole { principal_id, role } => {
            sqlx::query(
                "UPDATE core.tenant_member SET role = $3, updated_at = now() \
                  WHERE tenant_id = $1 AND principal_id = $2",
            )
            .bind(tenant_id)
            .bind(principal_id)
            .bind(role.as_str())
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
        }
        MemberOp::Remove { principal_id } => {
            sqlx::query(
                "DELETE FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
            )
            .bind(tenant_id)
            .bind(principal_id)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;

            // S01-W13：级联撤销该真人在本租户内的**全部真人项目成员关系**。
            // 少了这一步，退出租户的人仍能访问其所有项目——`require_project_member`
            // 只查 `core.project_member`，不查 `core.tenant_member`。
            // Agent 职能行不在级联范围（租户级实例，bootstrap 维护）。
            let actor = AuditActor {
                principal_id: seen.id,
                identity_id: seen.identity_id,
                client: seen.client.clone(),
                auth_mode: seen.auth_mode.clone(),
                claimed_device_id: None,
            };
            let revoked_project_ids =
                revoke_principal_memberships(&mut **tx, tenant_id, principal_id, &actor)
                    .await
                    .map_err(|err| {
                        tracing::error!(error = %err, "tenant member cascade revoke failed");
                        db_error(err)
                    })?;
            return Ok(WriteOutcome {
                project_memberships_revoked: revoked_project_ids.len(),
                revoked_project_ids,
            });
        }
        MemberOp::TransferOwner { new_owner } => {
            sqlx::query(
                "UPDATE core.tenant_member SET role = 'admin', updated_at = now() \
                  WHERE tenant_id = $1 AND principal_id = $2",
            )
            .bind(tenant_id)
            .bind(actor_id)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
            sqlx::query(
                "UPDATE core.tenant_member SET role = 'owner', updated_at = now() \
                  WHERE tenant_id = $1 AND principal_id = $2",
            )
            .bind(tenant_id)
            .bind(new_owner)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
        }
    }
    Ok(WriteOutcome::default())
}

/// 一次变更要写的审计行（owner 转让会产出两条）。
struct AuditLine {
    action: &'static str,
    target: Uuid,
    before: Option<TenantRole>,
    after: Option<TenantRole>,
    /// 附加到 `after_data` 的额外字段（如级联撤销的项目成员关系数）。
    after_extra: Option<Value>,
}

struct Plan {
    outcome: Value,
    audits: Vec<AuditLine>,
}

/// 纯规则判定：能力矩阵 + 最后 owner 保护 + owner 转让独占。
fn authorize(
    actor_role: TenantRole,
    op: MemberOp,
    target_role: Option<TenantRole>,
    owner_count: i64,
    actor_id: Uuid,
) -> Result<Plan, Response> {
    match op {
        MemberOp::Add { principal_id, role } => {
            if role == TenantRole::Owner {
                return Err(conflict("USE_OWNER_TRANSFER", "use POST .../owner-transfer"));
            }
            if role == TenantRole::Admin && actor_role != TenantRole::Owner {
                return Err(err_forbidden("admin cannot grant admin"));
            }
            if target_role.is_some() {
                return Err(conflict("ALREADY_MEMBER", "principal is already a member"));
            }
            Ok(Plan {
                outcome: json!({ "principal_id": principal_id, "role": role.as_str() }),
                audits: vec![AuditLine {
                    action: "tenant_member.added",
                    target: principal_id,
                    before: None,
                    after: Some(role),
                    after_extra: None,
                }],
            })
        }

        MemberOp::SetRole { principal_id, role } => {
            if role == TenantRole::Owner {
                return Err(conflict("USE_OWNER_TRANSFER", "use POST .../owner-transfer"));
            }
            let current = target_role.ok_or_else(|| err_not_found("member not found"))?;

            if actor_role == TenantRole::Admin {
                // admin 只能往下降（含把另一个 admin 降为 member），不能提权、
                // 不能动 owner、不能改自己。
                if current == TenantRole::Owner {
                    return Err(err_forbidden("admin cannot change an owner"));
                }
                if role == TenantRole::Admin {
                    return Err(err_forbidden("admin cannot grant admin"));
                }
                if principal_id == actor_id {
                    return Err(err_forbidden("admin cannot change own role"));
                }
            }

            if current == TenantRole::Owner && role != TenantRole::Owner && owner_count <= 1 {
                return Err(conflict("OWNER_LAST", "the last owner cannot be demoted"));
            }
            if current == role {
                return Ok(Plan {
                    outcome: json!({ "principal_id": principal_id, "role": role.as_str() }),
                    audits: vec![],
                });
            }
            Ok(Plan {
                outcome: json!({ "principal_id": principal_id, "role": role.as_str() }),
                audits: vec![AuditLine {
                    action: "tenant_member.role_changed",
                    target: principal_id,
                    before: Some(current),
                    after: Some(role),
                    after_extra: None,
                }],
            })
        }

        MemberOp::Remove { principal_id } => {
            let current = target_role.ok_or_else(|| err_not_found("member not found"))?;
            if current == TenantRole::Owner {
                if owner_count <= 1 {
                    return Err(conflict("OWNER_LAST", "the last owner cannot be removed"));
                }
                if actor_role != TenantRole::Owner {
                    return Err(err_forbidden("only an owner can remove an owner"));
                }
            }
            Ok(Plan {
                outcome: json!({ "principal_id": principal_id, "removed": true }),
                audits: vec![AuditLine {
                    action: "tenant_member.removed",
                    target: principal_id,
                    before: Some(current),
                    after: None,
                    after_extra: None,
                }],
            })
        }

        MemberOp::TransferOwner { new_owner } => {
            if !actor_role.can_transfer_ownership() {
                return Err(err_forbidden("only an owner can transfer ownership"));
            }
            let new_role = target_role.ok_or_else(|| err_not_found("member not found"))?;
            if new_owner == actor_id {
                return Err(conflict("ALREADY_OWNER", "caller is already the owner"));
            }
            if new_role == TenantRole::Owner {
                return Err(conflict("ALREADY_OWNER", "target is already an owner"));
            }
            Ok(Plan {
                outcome: json!({
                    "previous_owner": actor_id,
                    "new_owner": new_owner,
                }),
                audits: vec![
                    AuditLine {
                        action: "tenant_member.owner_transferred",
                        target: actor_id,
                        before: Some(TenantRole::Owner),
                        after: Some(TenantRole::Admin),
                        after_extra: None,
                    },
                    AuditLine {
                        action: "tenant_member.owner_transferred",
                        target: new_owner,
                        before: Some(new_role),
                        after: Some(TenantRole::Owner),
                        after_extra: None,
                    },
                ],
            })
        }
    }
}

struct ActorRecord {
    id: Uuid,
    identity_id: Option<Uuid>,
    client: Option<String>,
    auth_mode: Option<String>,
}

fn actor_record(caller: &AuthedPrincipal) -> ActorRecord {
    let actor = caller.audit_actor();
    ActorRecord {
        id: actor.principal_id,
        identity_id: actor.identity_id,
        client: actor.client,
        auth_mode: actor.auth_mode,
    }
}

async fn insert_audit(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    actor: &ActorRecord,
    line: &AuditLine,
) -> Result<(), Response> {
    sqlx::query(
        "INSERT INTO governance.tenant_audit_log \
         (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_principal_id, before_data, after_data) \
         VALUES ($1, $2, $3, $4, $5, NULL, $6, $7, $8, $9)",
    )
    .bind(tenant_id)
    .bind(actor.id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(line.action)
    .bind(line.target)
    .bind(line.before.map(|role| json!({ "role": role.as_str() })))
    .bind(after_data(line))
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(())
}

/// `after_data` = 角色快照 + 可选附加字段（如级联撤销计数）。
fn after_data(line: &AuditLine) -> Option<Value> {
    let mut map = serde_json::Map::new();
    if let Some(role) = line.after {
        map.insert("role".to_string(), json!(role.as_str()));
    }
    if let Some(Value::Object(extra)) = &line.after_extra {
        for (key, value) in extra {
            map.insert(key.clone(), value.clone());
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
}

// ─────────────────────────────────────────────────────────── 错误构造

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
