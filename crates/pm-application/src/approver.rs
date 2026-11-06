//! S02-I6-A · Approver 显式授权基座（O-6 裁决落地，迁移 0040）。
//!
//! 授权矩阵（O-6 配套裁决，Q-A1「Lead+显式 Approver」宽读）：
//! - **confirm/reject/withdraw/restore/archive** = 真人主体 + 项目成员 +
//!   （Lead(`project_member.role='owner'`) OR 本模块维护的生效授权行）；
//! - **grant/revoke/list** = Lead 专属；被授人须为在职成员且非现任 Lead
//!   （Lead 本身已持权，授权行仅服务于非 Lead 的显式 Approver）；
//! - 载体 = `core.approver_grant` 单行状态表（每 (project, principal) 一行，
//!   revoked_at 置空即生效；重复授予=复活行）；历史走 audit_log。
//!
//! 偏差注记：S02-04 §7.4 严格读法（Lead 亦须显式授权）不采纳——新项目会
//! 陷入无人可确认的冷启动死锁。step-up 确认票据（A* 补齐路径，H-17/H-18
//! 门禁）另列后续项。MCP 零确认工具维持（R-I6A-1 处置 (a)，H-15）。
//!
//! 审计词表：`approver.granted` / `approver.revoked`（S02D-23 点号式的
//! I6-A 扩展；target_type='approver_grant'，target_id=被授 principal）。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::{append_audit, PmWriteError};

/// 生效授权视图（列表/详情）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct ApproverGrantView {
    pub principal_id: Uuid,
    pub display_name: Option<String>,
    pub granted_by: Uuid,
    pub granted_by_name: Option<String>,
    pub granted_at: DateTime<Utc>,
}

/// 是否项目 Lead（role='owner'）。
pub async fn is_project_lead(
    db: impl sqlx::PgExecutor<'_>,
    project_id: Uuid,
    principal_id: Uuid,
) -> Result<bool, PmWriteError> {
    let found: Option<Uuid> = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
         WHERE project_id = $1 AND principal_id = $2 AND role = 'owner' LIMIT 1",
    )
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(db)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(found.is_some())
}

/// 确认/治理权限闸（requirement 域唯一入口）：真人主体由调用方先验，
/// 这里只判 **Lead OR 生效 Approver 授权**。
pub async fn ensure_governance_authorized(
    pool: &PgPool,
    project_id: Uuid,
    principal_id: Uuid,
) -> Result<(), PmWriteError> {
    if is_project_lead(pool, project_id, principal_id).await? {
        return Ok(());
    }
    let granted: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM core.approver_grant \
         WHERE project_id = $1 AND principal_id = $2 AND revoked_at IS NULL LIMIT 1",
    )
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?;
    if granted.is_some() {
        return Ok(());
    }
    Err(PmWriteError::Forbidden(
        "this action requires the project lead or an explicit approver grant",
    ))
}

/// 授予 Approver（Lead 专属）：被授人须为在职成员且非现任 Lead；
/// 已撤销行复活（granted_by/granted_at 刷新）。
pub async fn grant_approver(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    grantee: Uuid,
    actor: &AuditActor,
) -> Result<ApproverGrantView, PmWriteError> {
    if grantee == actor.principal_id {
        return Err(PmWriteError::Validation(
            "cannot grant approver to yourself (the lead already holds the right)",
        ));
    }
    let mut tx = pool.begin().await?;
    if !is_project_lead(&mut *tx, project_id, actor.principal_id).await? {
        return Err(PmWriteError::Forbidden(
            "approver grants are managed by the project lead",
        ));
    }
    // 被授人须为在职成员（DEF-01：确认是项目域写操作，无成员行的主体不收）。
    let member: Option<Uuid> = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
         WHERE project_id = $1 AND principal_id = $2 LIMIT 1",
    )
    .bind(project_id)
    .bind(grantee)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if member.is_none() {
        return Err(PmWriteError::Validation(
            "approver grantee must be an active project member",
        ));
    }
    // 被授人非现任 Lead（Lead 已持权；避免出现语义空转的授权行）。
    if is_project_lead(&mut *tx, project_id, grantee).await? {
        return Err(PmWriteError::Validation(
            "the project lead already holds confirmation rights",
        ));
    }

    sqlx::query(
        "INSERT INTO core.approver_grant \
         (project_id, principal_id, granted_by, tenant_id) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (project_id, principal_id) DO UPDATE \
         SET revoked_at = NULL, revoked_by = NULL, granted_by = $3, granted_at = now()",
    )
    .bind(project_id)
    .bind(grantee)
    .bind(actor.principal_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "approver.granted",
        "approver_grant",
        Some(grantee),
    )
    .await?;
    tx.commit().await?;

    view_of(pool, project_id, grantee)
        .await?
        .ok_or(PmWriteError::NotFound("approver grant not found after grant"))
}

/// 撤销 Approver（Lead 专属）：仅对生效行操作；无生效行 404。
pub async fn revoke_approver(
    pool: &PgPool,
    project_id: Uuid,
    grantee: Uuid,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    let mut tx = pool.begin().await?;
    if !is_project_lead(&mut *tx, project_id, actor.principal_id).await? {
        return Err(PmWriteError::Forbidden(
            "approver grants are managed by the project lead",
        ));
    }
    let revoked: Option<Uuid> = sqlx::query_scalar(
        "UPDATE core.approver_grant \
         SET revoked_at = now(), revoked_by = $3 \
         WHERE project_id = $1 AND principal_id = $2 AND revoked_at IS NULL \
         RETURNING id",
    )
    .bind(project_id)
    .bind(grantee)
    .bind(actor.principal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if revoked.is_none() {
        return Err(PmWriteError::NotFound("no active approver grant"));
    }

    append_audit(
        &mut tx,
        project_id,
        actor,
        "approver.revoked",
        "approver_grant",
        Some(grantee),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 生效 Approver 列表（成员可见）。
pub async fn list_approvers(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<ApproverGrantView>, PmWriteError> {
    sqlx::query_as(
        "SELECT g.principal_id, p.display_name, g.granted_by, gb.display_name AS granted_by_name, \
                g.granted_at \
         FROM core.approver_grant g \
         LEFT JOIN core.principal p ON p.id = g.principal_id \
         LEFT JOIN core.principal gb ON gb.id = g.granted_by \
         WHERE g.project_id = $1 AND g.revoked_at IS NULL \
         ORDER BY g.granted_at",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}

async fn view_of(
    pool: &PgPool,
    project_id: Uuid,
    grantee: Uuid,
) -> Result<Option<ApproverGrantView>, PmWriteError> {
    sqlx::query_as(
        "SELECT g.principal_id, p.display_name, g.granted_by, gb.display_name AS granted_by_name, \
                g.granted_at \
         FROM core.approver_grant g \
         LEFT JOIN core.principal p ON p.id = g.principal_id \
         LEFT JOIN core.principal gb ON gb.id = g.granted_by \
         WHERE g.project_id = $1 AND g.principal_id = $2 AND g.revoked_at IS NULL",
    )
    .bind(project_id)
    .bind(grantee)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)
}
