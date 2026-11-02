//! 设备 ↔ 项目绑定（T-088）。
//!
//! Binding 是**声明与事实记录**，**不是权限授予机制**（Codex Bootstrap V1.3
//! §5.4.1、评审 P0-2）：绑定不扩大 Project / Repository 权限——真人的权限仍由
//! 他自己的成员资格与仓库权限决定。
//!
//! 并发与幂等（设计稿 §7.6 / §8.4）：
//! - 重复绑定**不产生重复行**（`UNIQUE(device_id, project_id)`）；
//! - **原有有效绑定不被重新初始化清除**；
//! - 解绑是软删除（置 `unbound_at`，行保留供审计与历史）；
//! - 重绑**复活原行**，且**在同一事务内**清空 `verified_repositories` 并更新声明。

use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

use super::{BindingRow, DeviceError, device_owner};
use crate::actor::AuditActor;
use crate::authz::require_project_member;
use crate::caller::HumanActor;

/// 绑定项目（幂等）。
///
/// `declared_repository_ids` 必须是该 Project 的 `integration.project_repository.id`
/// 子集——服务端校验，不信任客户端。
pub async fn bind_project(
    pool: &PgPool,
    caller: &HumanActor,
    device_id: Uuid,
    project_id: Uuid,
    declared_repository_ids: &[Uuid],
) -> Result<BindingRow, DeviceError> {
    let (tenant_id, owner) = device_owner(pool, device_id)
        .await?
        .ok_or(DeviceError::NotFound)?;
    if tenant_id != caller.tenant_id || owner != caller.principal_id {
        return Err(DeviceError::NotFound);
    }
    // 绑定人必须是对该项目的成员——绑定由有权限的成员确认，不由 Skill 自行决定。
    require_project_member(pool, caller.principal_id, caller.tenant_id, project_id)
        .await
        .map_err(|_| DeviceError::NotFound)?;

    // 声明的仓库必须属于该项目（服务端校验项目归属）。
    let project_repos = project_repository_ids(pool, project_id).await?;
    for repo in declared_repository_ids {
        if !project_repos.contains(repo) {
            return Err(DeviceError::Validation(format!(
                "repository {repo} does not belong to project {project_id}"
            )));
        }
    }

    let mut tx = pool.begin().await.map_err(|e| DeviceError::Db(e.to_string()))?;

    // 已有行（含已解绑）→ 复活并更新；声明变化 → 旧验证结果失效（同一事务）。
    let existing = sqlx::query(
        "SELECT id, tenant_id, device_id, project_id, declared_repository_ids, \
                verified_repositories, bound_by, bound_at, unbound_at \
         FROM devenv.device_project_binding \
         WHERE device_id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(device_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;

    let declared_json = serde_json::to_value(declared_repository_ids)
        .map_err(|e| DeviceError::Db(e.to_string()))?;

    let binding_id = match existing {
        Some(row) => {
            let id: Uuid = row.get("id");
            let was_unbound =
                row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("unbound_at").is_some();
            let old_declared: Vec<Uuid> = super::uuid_list(row.get("declared_repository_ids"));
            let declarations_changed = old_declared != declared_repository_ids.to_vec();
            // 复活（曾解绑）或声明变化 → 清空验证结果；否则保留既有验证结果。
            let keep_verified = !was_unbound && !declarations_changed;
            sqlx::query(
                "UPDATE devenv.device_project_binding \
                 SET declared_repository_ids = $2, \
                     verified_repositories = CASE WHEN $3 THEN verified_repositories \
                                                 ELSE '[]'::jsonb END, \
                     bound_by = $4, bound_at = now(), unbound_at = NULL \
                 WHERE id = $1",
            )
            .bind(id)
            .bind(&declared_json)
            .bind(keep_verified)
            .bind(caller.principal_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DeviceError::Db(e.to_string()))?;
            id
        }
        None => {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO devenv.device_project_binding \
                 (id, tenant_id, device_id, project_id, declared_repository_ids, \
                  verified_repositories, bound_by, bound_at) \
                 VALUES ($1, $2, $3, $4, $5, '[]'::jsonb, $6, now())",
            )
            .bind(id)
            .bind(caller.tenant_id)
            .bind(device_id)
            .bind(project_id)
            .bind(&declared_json)
            .bind(caller.principal_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DeviceError::Db(e.to_string()))?;
            id
        }
    };

    append_binding_audit(
        &mut tx,
        caller,
        project_id,
        "device.project_bound",
        binding_id,
        device_id,
    )
    .await?;

    let row = sqlx::query(
        "SELECT id, tenant_id, device_id, project_id, declared_repository_ids, \
                verified_repositories, bound_by, bound_at, unbound_at \
         FROM devenv.device_project_binding WHERE id = $1",
    )
    .bind(binding_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;

    tx.commit().await.map_err(|e| DeviceError::Db(e.to_string()))?;

    Ok(BindingRow {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        device_id: row.get("device_id"),
        project_id: row.get("project_id"),
        declared_repository_ids: super::uuid_list(row.get("declared_repository_ids")),
        verified_repositories: super::uuid_list(row.get("verified_repositories")),
        bound_by: row.get("bound_by"),
        bound_at: row.get("bound_at"),
        unbound_at: row.get("unbound_at"),
    })
}

/// 解绑（软删除：行保留，`unbound_at` 置位）。
pub async fn unbind_binding(
    pool: &PgPool,
    caller: &HumanActor,
    device_id: Uuid,
    binding_id: Uuid,
) -> Result<(), DeviceError> {
    let (tenant_id, owner) = device_owner(pool, device_id)
        .await?
        .ok_or(DeviceError::NotFound)?;
    if tenant_id != caller.tenant_id || owner != caller.principal_id {
        return Err(DeviceError::NotFound);
    }
    let mut tx = pool.begin().await.map_err(|e| DeviceError::Db(e.to_string()))?;
    let row = sqlx::query(
        "SELECT project_id FROM devenv.device_project_binding \
         WHERE id = $1 AND device_id = $2 AND unbound_at IS NULL FOR UPDATE",
    )
    .bind(binding_id)
    .bind(device_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?
    .ok_or(DeviceError::BindingNotFound)?;
    let project_id: Uuid = row.get("project_id");

    sqlx::query("UPDATE devenv.device_project_binding SET unbound_at = now() WHERE id = $1")
        .bind(binding_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DeviceError::Db(e.to_string()))?;

    append_binding_audit(
        &mut tx,
        caller,
        project_id,
        "device.project_unbound",
        binding_id,
        device_id,
    )
    .await?;

    tx.commit().await.map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(())
}

/// 该项目的仓库 id 集合（`integration.project_repository.id`）。
pub(crate) async fn project_repository_ids(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<Uuid>, DeviceError> {
    let rows = sqlx::query_scalar("SELECT id FROM integration.project_repository WHERE project_id = $1")
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(rows)
}

/// 绑定类事件的审计（项目级 `governance.audit_log`；执行主体 = 真人——REST 路径）。
async fn append_binding_audit(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    caller: &HumanActor,
    project_id: Uuid,
    action: &str,
    binding_id: Uuid,
    device_id: Uuid,
) -> Result<(), DeviceError> {
    let actor = AuditActor {
        principal_id: caller.principal_id,
        identity_id: caller.identity_id,
        client: caller.client.clone(),
        auth_mode: Some(caller.auth_mode.clone()),
        claimed_device_id: Some(device_id),
    };
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, \
          claimed_device_id, action, target_type, target_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'device_binding', $9, $10)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(binding_id)
    .bind(caller.tenant_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(())
}
