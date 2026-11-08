//! Task ↔ Repository 绑定的生产写入口（WP-C / V0.5 roadmap §8）。
//!
//! 背景（V2 代码核验 P0 空洞）：`delivery.task_repository` 全仓零生产 INSERT——
//! 只有测试种子手写 SQL，生产任务永远拿不到仓库关联 → `repo_access` 快照恒空
//! → 执行授权 deny-all。本模块补齐写入口，六约束逐条落位：
//!
//! 1. **关联只来自当前 Project 的有效 Repository**（`integration.project_repository`
//!    同租户同项目 + `status='verified'`——与读方 [`crate::repo_access`] 的
//!    「Task 关联 ∩ verified」同一判定，不另造第二套有效性谓词）；
//! 2. **创建关联时校验既有授权**：仓库必须先经仓库管理面验证（S01-03 verified
//!    才算可用）；角色域复用 0011 的 CHECK（primary/secondary/knowledge）；
//! 3. **不允许跨 Project 写入**：仓库存在但属其他项目 → 显式
//!    [`TaskRepoBindingError::RepositoryNotInProject`]（不泄露目标仓库存在性
//!    之外的信息，与 task_dependency 的 CROSS_PROJECT 同规格）；
//! 4. **重复写入幂等**：PK `(task_id, repository_id)` + `ON CONFLICT DO UPDATE`
//!    （未提供字段 COALESCE 保留现值）——同输入重复调用收敛到同一状态，不产重复行；
//! 5. **Claim 与 execution_allows 消费同一关联事实**：两者读的都是本表
//!    （claim 快照 = [`crate::repo_access::build_task_repo_access_tx`]，执行期
//!    = [`crate::repo_access::execution_allows`]），本模块只写、不另建授权缓存；
//! 6. **不允许经 MCP 参数伪造归属**：MCP 四工具无仓库参数；归属只能经本服务
//!    写入（REST/UI），且执行期对"同项目 verified 但未绑定"的仓库 deny
//!    （快照缺失 → [`crate::repo_access::snapshot_allows`] = false）。
//!
//! Run 快照冻结语义：绑定变更只影响**未来**的 claim（`task_run_repository` 在
//! 领取时固化），不回写既有 Run——执行期撤销走 unbind + 仓库状态治理。

use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit_with_after;

/// 绑定角色域（0011 CHECK 原样，不在本模块重新裁量）。
pub const BINDING_ROLES: [&str; 3] = ["primary", "secondary", "knowledge"];

#[derive(Debug, thiserror::Error)]
pub enum TaskRepoBindingError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    /// 任务不存在 / 跨租户（不泄露存在性）。
    #[error("task not found in this project")]
    TaskNotFound,
    /// 仓库不存在 / 跨租户。
    #[error("repository not found")]
    RepositoryNotFound,
    /// 仓库存在但属于其他项目（约束 3；仿 task_dependency CROSS_PROJECT 规格）。
    #[error("repository belongs to another project")]
    RepositoryNotInProject,
    /// 仓库未验证（约束 1/2：有效仓库 = verified）。
    #[error("repository is not verified (current status: {0})")]
    RepositoryNotVerified(String),
    #[error("invalid role `{0}` (expected one of {BINDING_ROLES:?})")]
    InvalidRole(String),
    #[error("{0}")]
    Validation(String),
}

/// 一条绑定入参（未提供的字段：新建用默认值，冲突时保留现值）。
#[derive(Debug, Clone)]
pub struct BindRepositoryEntry {
    pub repository_id: Uuid,
    pub write_access: Option<bool>,
    pub base_ref: Option<String>,
    pub role: Option<String>,
    pub path_scope: Option<Vec<String>>,
}

/// 绑定行（GET/POST 响应元素；`repository_name` 供 UI 直显；`repository_verified`
/// 标注"已绑定但快照不生效"的行——快照只取 verified，见 [`crate::repo_access`]）。
#[derive(Debug, Clone, Serialize)]
pub struct TaskRepositoryBinding {
    pub task_id: Uuid,
    pub repository_id: Uuid,
    pub repository_name: String,
    pub write_access: bool,
    pub base_ref: String,
    pub role: String,
    pub path_scope: Option<Vec<String>>,
    pub repository_verified: bool,
}

/// 为 Task 绑定仓库（幂等 upsert；约束 1–4）。
///
/// 事务内：Task 存在性 → 逐条（角色域 → 仓库归属 → verified → upsert → 审计）。
pub async fn bind_task_repositories(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    entries: &[BindRepositoryEntry],
    actor: &AuditActor,
) -> Result<Vec<TaskRepositoryBinding>, TaskRepoBindingError> {
    if entries.is_empty() {
        return Err(TaskRepoBindingError::Validation(
            "at least one repository entry is required".into(),
        ));
    }
    for e in entries {
        if let Some(role) = e.role.as_deref() {
            if !BINDING_ROLES.contains(&role) {
                return Err(TaskRepoBindingError::InvalidRole(role.to_string()));
            }
        }
    }

    let mut tx = pool.begin().await?;

    // 约束 3（任务侧）：Task 必须属于本租户本项目。
    let task_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM delivery.task WHERE id = $1 AND project_id = $2")
            .bind(task_id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
    if task_tenant.is_none() || task_tenant != Some(tenant_id) {
        return Err(TaskRepoBindingError::TaskNotFound);
    }

    for e in entries {
        // 约束 1/3（仓库侧）：同租户同项目内寻仓；跨项目显式拒绝。
        let repo = sqlx::query(
            "SELECT name, status FROM integration.project_repository \
              WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(e.repository_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(repo) = repo else {
            let anywhere: Option<Uuid> = sqlx::query_scalar(
                "SELECT project_id FROM integration.project_repository \
                  WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant_id)
            .bind(e.repository_id)
            .fetch_optional(&mut *tx)
            .await?;
            return Err(match anywhere {
                Some(_) => TaskRepoBindingError::RepositoryNotInProject,
                None => TaskRepoBindingError::RepositoryNotFound,
            });
        };
        let status: String = repo.get("status");
        if status != "verified" {
            return Err(TaskRepoBindingError::RepositoryNotVerified(status));
        }

        // 约束 4：幂等 upsert（冲突时未提供字段保留现值）。
        sqlx::query(
            "INSERT INTO delivery.task_repository \
               (task_id, project_id, repository_id, write_access, base_ref, role, path_scope, tenant_id) \
             VALUES ($1, $2, $3, COALESCE($4, TRUE), COALESCE($5, 'main'), COALESCE($6, 'primary'), $7, $8) \
             ON CONFLICT (task_id, repository_id) DO UPDATE SET \
                write_access = COALESCE($4, delivery.task_repository.write_access), \
                base_ref     = COALESCE($5, delivery.task_repository.base_ref), \
                role         = COALESCE($6, delivery.task_repository.role), \
                path_scope   = COALESCE($7, delivery.task_repository.path_scope)",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(e.repository_id)
        .bind(e.write_access)
        .bind(e.base_ref.as_deref())
        .bind(e.role.as_deref())
        .bind(e.path_scope.as_deref())
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;

        append_audit_with_after(
            &mut tx,
            project_id,
            actor,
            "task.repository.bound",
            "task",
            Some(task_id),
            serde_json::json!({
                "repository_id": e.repository_id,
                "write_access": e.write_access,
                "base_ref": e.base_ref,
                "role": e.role,
                "path_scope": e.path_scope,
            }),
        )
        .await?;
    }

    tx.commit().await?;

    // 收尾回读（事务外；行已由 upsert 保证存在）。
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        out.push(fetch_binding(pool, tenant_id, project_id, task_id, e.repository_id).await?);
    }
    Ok(out)
}

/// 解除绑定（约束 4 的逆操作；幂等——不存在也返回 false 而非报错）。
pub async fn unbind_task_repository(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    repository_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, TaskRepoBindingError> {
    let mut tx = pool.begin().await?;
    let task_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM delivery.task WHERE id = $1 AND project_id = $2")
            .bind(task_id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
    if task_tenant.is_none() || task_tenant != Some(tenant_id) {
        return Err(TaskRepoBindingError::TaskNotFound);
    }
    let deleted = sqlx::query(
        "DELETE FROM delivery.task_repository \
          WHERE task_id = $1 AND repository_id = $2 AND project_id = $3 AND tenant_id = $4",
    )
    .bind(task_id)
    .bind(repository_id)
    .bind(project_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if deleted > 0 {
        append_audit_with_after(
            &mut tx,
            project_id,
            actor,
            "task.repository.unbound",
            "task",
            Some(task_id),
            serde_json::json!({ "repository_id": repository_id }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(deleted > 0)
}

/// 列出 Task 的全部绑定（GET；读方快照 = 本表 ∩ verified，此处如实回显含未验证行，
/// `repository_verified` 标注"绑定了但快照不生效"的行）。
pub async fn list_task_repositories(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<TaskRepositoryBinding>, TaskRepoBindingError> {
    let rows = sqlx::query(
        "SELECT tr.task_id, tr.repository_id, r.name, tr.write_access, tr.base_ref, tr.role, \
                tr.path_scope, (r.status = 'verified') AS repository_verified \
           FROM delivery.task_repository tr \
           JOIN integration.project_repository r \
             ON r.tenant_id = tr.tenant_id AND r.project_id = tr.project_id \
            AND r.id = tr.repository_id \
          WHERE tr.tenant_id = $1 AND tr.project_id = $2 AND tr.task_id = $3 \
          ORDER BY tr.created_at, tr.repository_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(binding_from_row).collect())
}

async fn fetch_binding(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    repository_id: Uuid,
) -> Result<TaskRepositoryBinding, TaskRepoBindingError> {
    let row = sqlx::query(
        "SELECT tr.task_id, tr.repository_id, r.name, tr.write_access, tr.base_ref, tr.role, \
                tr.path_scope, (r.status = 'verified') AS repository_verified \
           FROM delivery.task_repository tr \
           JOIN integration.project_repository r \
             ON r.tenant_id = tr.tenant_id AND r.project_id = tr.project_id \
            AND r.id = tr.repository_id \
          WHERE tr.tenant_id = $1 AND tr.project_id = $2 AND tr.task_id = $3 AND tr.repository_id = $4",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(task_id)
    .bind(repository_id)
    .fetch_one(pool)
    .await?;
    Ok(binding_from_row(row))
}

fn binding_from_row(row: sqlx::postgres::PgRow) -> TaskRepositoryBinding {
    TaskRepositoryBinding {
        task_id: row.get("task_id"),
        repository_id: row.get("repository_id"),
        repository_name: row.get("name"),
        write_access: row.get("write_access"),
        base_ref: row.get("base_ref"),
        role: row.get("role"),
        path_scope: row.get("path_scope"),
        repository_verified: row.get("repository_verified"),
    }
}
