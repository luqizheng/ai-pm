//! Project 成员授权（S01-W13，`docs/ai-pm-s01-project-member-authorization-v1.md`）。
//!
//! `core.project_member` 此前**只读**——除 bootstrap 外全仓零写入路径，
//! 人类无法被授予或撤销项目访问。本模块补齐写侧，并修掉「租户撤权不级联」
//! 的越权洞：移除 `core.tenant_member` 行后，该人此前仍是各项目的
//! `core.project_member`，仍可访问（`require_project_member` 只查后者）。
//!
//! 三条硬规则：
//!
//! 1. **授权主体永远是真人**——`core.principal.kind='human'`；Agent 职能行
//!    （`ai_pm` / `architect` / `developer` / `tester`）由 bootstrap 独占维护。
//! 2. **锁序固定** `core.tenant → core.project → core.project_member`，
//!    不碰 run/task 行（与 I1 的 `project → run → task` 无环）。
//! 3. **审计与写入同事务**——审计写失败即整体回滚。

use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use pm_domain::{ProjectMemberRole, TenantRole, is_agent_role};

use crate::actor::AuditActor;
use crate::pm_write::append_audit;

/// Project 成员授权失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectMemberError {
    /// 目标不存在或对调用人不可见（上层映射 404，不泄露存在性）。
    #[error("not found: {0}")]
    NotFound(&'static str),
    /// 调用人不是该项目的管理者。
    #[error("forbidden: {0}")]
    Forbidden(&'static str),
    /// 业务冲突。
    #[error("{code}: {message}")]
    Conflict {
        code: &'static str,
        message: &'static str,
    },
    /// 数据库错误。
    #[error("database error: {0}")]
    Db(String),
}

/// 一次成员变更操作。
#[derive(Debug, Clone, Copy)]
pub enum MemberOp {
    /// 授予项目成员（角色固定 `member`；`owner` 只能走所有权转移）。
    Grant { principal_id: Uuid },
    /// 撤销项目成员。
    Revoke { principal_id: Uuid },
    /// 转移项目所有权。
    TransferOwner { new_owner: Uuid },
}

impl MemberOp {
    /// 操作对象 principal（授予 / 撤销的目标，或所有权转移的新 owner）。
    pub fn target(self) -> Uuid {
        match self {
            Self::Grant { principal_id }
            | Self::Revoke { principal_id } => principal_id,
            Self::TransferOwner { new_owner } => new_owner,
        }
    }
}

/// 一次成员变更的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberChangeOutcome {
    /// 操作对象 principal。
    pub principal_id: Uuid,
    /// 变更后的角色（撤销时为 `None`）。
    pub role: Option<ProjectMemberRole>,
    /// 是否为撤销。
    pub revoked: bool,
    /// 所有权转移的原 owner。
    pub previous_owner: Option<Uuid>,
}

/// 项目成员行（列表视图）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRow {
    pub principal_id: Uuid,
    pub display_name: String,
    /// `core.project_member.role` 原值——可能是真人角色，也可能是 Agent 职能值。
    pub role: String,
    /// `core.principal.kind`：`human` / `dev_agent` / `ai_pm` / `service`。
    pub kind: String,
    pub is_active: bool,
}

/// 列项目成员（含 Agent 职能行；调用方按 `kind` 区分）。
pub async fn list_members(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<MemberRow>, ProjectMemberError> {
    let rows = sqlx::query(
        "SELECT m.principal_id, p.display_name, m.role, p.kind, p.is_active \
           FROM core.project_member m \
           JOIN core.principal p ON p.id = m.principal_id \
          WHERE m.tenant_id = $1 AND m.project_id = $2 \
          ORDER BY CASE m.role WHEN 'owner' THEN 0 ELSE 1 END, p.display_name",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ProjectMemberError::Db(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| MemberRow {
            principal_id: r.get("principal_id"),
            display_name: r.get("display_name"),
            role: r.get("role"),
            kind: r.get("kind"),
            is_active: r.get("is_active"),
        })
        .collect())
}

/// 执行一次成员变更（单事务：锁 → 校验 → 写 → 审计）。
pub async fn apply_member_change(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    op: MemberOp,
) -> Result<MemberChangeOutcome, ProjectMemberError> {
    let mut tx = pool.begin().await.map_err(db_err)?;

    // 1) 锁租户行：把同一租户内跨项目的成员变更串行化——授权前提校验
    //    （目标必须是租户成员）与租户撤权级联必须在同一把锁下互斥。
    let tenant_locked: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.tenant WHERE id = $1 FOR UPDATE")
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
    if tenant_locked.is_none() {
        return Err(ProjectMemberError::NotFound("tenant not found"));
    }

    // 2) 锁项目行：把同一项目内的成员变更串行化（owner 唯一性、并发授予/撤销）。
    let project_locked: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM core.project WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_err)?;
    if project_locked.is_none() {
        return Err(ProjectMemberError::NotFound("project not found"));
    }

    // 3) 判定调用人权限（实时查库，不读 token）。
    if !is_manager(&mut tx, tenant_id, project_id, actor.principal_id).await? {
        return Err(ProjectMemberError::Forbidden("project owner required"));
    }

    let outcome = match op {
        MemberOp::Grant { principal_id } => grant(&mut tx, tenant_id, project_id, actor, principal_id).await?,
        MemberOp::Revoke { principal_id } => revoke(&mut tx, project_id, actor, principal_id).await?,
        MemberOp::TransferOwner { new_owner } => {
            transfer_owner(&mut tx, tenant_id, project_id, actor, new_owner).await?
        }
    };

    tx.commit().await.map_err(db_err)?;
    Ok(outcome)
}

/// 调用人是否是该项目的管理者：项目 owner **或** 租户 owner/admin。
async fn is_manager(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    actor_id: Uuid,
) -> Result<bool, ProjectMemberError> {
    let tenant_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2",
    )
    .bind(tenant_id)
    .bind(actor_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;

    if let Some(role) = tenant_role.as_deref().and_then(TenantRole::parse) {
        if role.is_tenant_admin() {
            return Ok(true);
        }
    }

    let project_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.project_member WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(project_id)
    .bind(actor_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;

    Ok(project_role
        .as_deref()
        .and_then(ProjectMemberRole::parse)
        .is_some_and(|role| role.is_manager()))
}

/// 授予项目成员。
async fn grant(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    actor: &AuditActor,
    principal_id: Uuid,
) -> Result<MemberChangeOutcome, ProjectMemberError> {
    let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
        .bind(principal_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_err)?;
    let kind = kind.ok_or(ProjectMemberError::NotFound("principal not found"))?;
    if kind != "human" {
        return Err(ProjectMemberError::Conflict {
            code: "AGENT_PRINCIPAL",
            message: "only human principals can be granted project membership",
        });
    }

    let is_tenant_member: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.tenant_member \
          WHERE tenant_id = $1 AND principal_id = $2)",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_err)?;
    if !is_tenant_member.unwrap_or(false) {
        return Err(ProjectMemberError::Conflict {
            code: "NOT_TENANT_MEMBER",
            message: "principal is not a member of this tenant",
        });
    }

    let existing: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.project_member \
          WHERE project_id = $1 AND principal_id = $2 FOR UPDATE",
    )
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;
    if existing.is_some() {
        return Err(ProjectMemberError::Conflict {
            code: "ALREADY_MEMBER",
            message: "principal is already a member of this project",
        });
    }

    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'member')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;

    append_audit(
        &mut *tx,
        project_id,
        actor,
        "project_member.granted",
        "project_member",
        Some(principal_id),
    )
    .await
    .map_err(db_err)?;

    Ok(MemberChangeOutcome {
        principal_id,
        role: Some(ProjectMemberRole::Member),
        revoked: false,
        previous_owner: None,
    })
}

/// 撤销项目成员。
async fn revoke(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    actor: &AuditActor,
    principal_id: Uuid,
) -> Result<MemberChangeOutcome, ProjectMemberError> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.project_member \
          WHERE project_id = $1 AND principal_id = $2 FOR UPDATE",
    )
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;
    let role = role.ok_or(ProjectMemberError::NotFound("member not found"))?;

    if is_agent_role(&role) {
        return Err(ProjectMemberError::Conflict {
            code: "AGENT_ROW",
            message: "agent rows are maintained by bootstrap and cannot be revoked here",
        });
    }
    if ProjectMemberRole::parse(&role) == Some(ProjectMemberRole::Owner) {
        return Err(ProjectMemberError::Conflict {
            code: "OWNER_LAST",
            message: "the project owner cannot be revoked; transfer ownership first",
        });
    }

    sqlx::query("DELETE FROM core.project_member WHERE project_id = $1 AND principal_id = $2")
        .bind(project_id)
        .bind(principal_id)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;

    append_audit(
        &mut *tx,
        project_id,
        actor,
        "project_member.revoked",
        "project_member",
        Some(principal_id),
    )
    .await
    .map_err(db_err)?;

    Ok(MemberChangeOutcome {
        principal_id,
        role: None,
        revoked: true,
        previous_owner: None,
    })
}

/// 转移项目所有权（原 owner 降为 member，新 owner 升为 owner）。
async fn transfer_owner(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    actor: &AuditActor,
    new_owner: Uuid,
) -> Result<MemberChangeOutcome, ProjectMemberError> {
    let previous_owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
          WHERE project_id = $1 AND role = 'owner' FOR UPDATE",
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;
    let previous_owner =
        previous_owner.ok_or(ProjectMemberError::NotFound("project has no owner"))?;

    if previous_owner == new_owner {
        return Err(ProjectMemberError::Conflict {
            code: "ALREADY_OWNER",
            message: "principal is already the project owner",
        });
    }

    let is_tenant_member: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.tenant_member \
          WHERE tenant_id = $1 AND principal_id = $2)",
    )
    .bind(tenant_id)
    .bind(new_owner)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_err)?;
    if !is_tenant_member.unwrap_or(false) {
        return Err(ProjectMemberError::Conflict {
            code: "NOT_TENANT_MEMBER",
            message: "principal is not a member of this tenant",
        });
    }

    let target_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.project_member \
          WHERE project_id = $1 AND principal_id = $2 FOR UPDATE",
    )
    .bind(project_id)
    .bind(new_owner)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err)?;
    let target_role = target_role.ok_or(ProjectMemberError::NotFound("member not found"))?;
    if is_agent_role(&target_role) {
        return Err(ProjectMemberError::Conflict {
            code: "AGENT_ROW",
            message: "agent rows cannot become project owner",
        });
    }

    sqlx::query("UPDATE core.project_member SET role = 'member' \
                  WHERE project_id = $1 AND principal_id = $2")
        .bind(project_id)
        .bind(previous_owner)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    sqlx::query("UPDATE core.project_member SET role = 'owner' \
                  WHERE project_id = $1 AND principal_id = $2")
        .bind(project_id)
        .bind(new_owner)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;

    for target in [previous_owner, new_owner] {
        append_audit(
            &mut *tx,
            project_id,
            actor,
            "project_member.owner_transferred",
            "project_member",
            Some(target),
        )
        .await
        .map_err(db_err)?;
    }

    Ok(MemberChangeOutcome {
        principal_id: new_owner,
        role: Some(ProjectMemberRole::Owner),
        revoked: false,
        previous_owner: Some(previous_owner),
    })
}

/// 该 principal 在本租户内担任 owner 的项目。
///
/// 租户成员移除前必须先查这个——否则会静默产生**无主项目**。
pub async fn owned_project_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT project_id FROM core.project_member \
          WHERE tenant_id = $1 AND principal_id = $2 AND role = 'owner'",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_all(pool)
    .await
}

/// 撤销该 principal 在本租户内的**全部真人**项目成员关系。
///
/// 供租户成员移除路径级联调用（`docs/ai-pm-s01-project-member-authorization-v1.md`
/// §2.4）：只删真人角色行（`owner` / `member`），**不动 Agent 职能行**——
/// Agent 是租户级实例（ADR-0001 D3），其项目授权由 bootstrap 维护。
///
/// 返回被撤销的项目 id 列表（逐项目写审计行）。
pub async fn revoke_principal_memberships(
    tx: &mut PgConnection,
    tenant_id: Uuid,
    principal_id: Uuid,
    actor: &AuditActor,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let project_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM core.project_member \
          WHERE tenant_id = $1 AND principal_id = $2 AND role IN ('owner', 'member') \
          FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_all(&mut *tx)
    .await?;

    if project_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query(
        "DELETE FROM core.project_member \
          WHERE tenant_id = $1 AND principal_id = $2 AND role IN ('owner', 'member')",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .execute(&mut *tx)
    .await?;

    for project_id in &project_ids {
        append_audit(
            tx,
            *project_id,
            actor,
            "project_member.revoked",
            "project_member",
            Some(principal_id),
        )
        .await?;
    }

    Ok(project_ids)
}

fn db_err(err: sqlx::Error) -> ProjectMemberError {
    ProjectMemberError::Db(err.to_string())
}

/// 供上层做「不可见即不存在」判定：调用人是否可见该项目成员列表。
///
/// 口径与 `support::project_visible` 一致，但**额外允许租户 admin**——
/// 租户 admin 是项目成员的管理者（设计稿 §2.3），必须能列出成员。
pub async fn caller_can_see_members(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    is_tenant_admin: bool,
) -> Result<bool, ProjectMemberError> {
    if is_tenant_admin {
        let exists: Option<bool> =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM core.project WHERE id = $1 AND tenant_id = $2)")
                .bind(project_id)
                .bind(tenant_id)
                .fetch_one(pool)
                .await
                .map_err(db_err)?;
        return Ok(exists.unwrap_or(false));
    }

    let visible: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS ( \
             SELECT 1 FROM core.project p \
             JOIN core.project_member m ON m.project_id = p.id \
              WHERE p.id = $1 AND p.tenant_id = $2 AND m.principal_id = $3)",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_one(pool)
    .await
    .map_err(db_err)?;

    Ok(visible.unwrap_or(false))
}
