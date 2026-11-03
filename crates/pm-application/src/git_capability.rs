//! Git 能力授权的存储与读取（S01 · I3-B，迁移 `0032`）。
//!
//! 判定规则在 [`pm_domain::git_capability`]（交集 + 依赖闭包，默认最小权限）；
//! 本模块只做三件事：
//!
//! 1. **授予 / 替换 / 撤销**——`PUT` 语义整体替换某主体在某仓的授权集，
//!    单事务 + 审计（`git_capability.granted` / `.revoked`）；
//! 2. **读取**——判定层每次请求实时查库（撤权即时生效，与 W13/I2 同口径）；
//! 3. **有效能力**——原始授权交给 [`pm_domain::effective_capabilities`] 做闭包。
//!
//! 管理权限：项目 owner 或租户 admin（与 S01-W13 同口径）。

use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use pm_domain::git_capability::GitCapability;

/// 一条授权（原始集——有效能力需再过依赖闭包）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct CapabilityGrant {
    pub repository_id: Uuid,
    pub repository_name: String,
    pub principal_id: Uuid,
    pub principal_name: String,
    pub principal_kind: String,
    /// 原始授权值（I3 §3.1 六能力字符串）。
    pub capabilities: Vec<String>,
    pub granted_by: Option<Uuid>,
}

/// 授权管理 / 判定失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GitCapabilityError {
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("forbidden: {0}")]
    Forbidden(&'static str),
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error("{0}")]
    Validation(String),
    #[error("database error: {0}")]
    Db(String),
}

/// 列出某仓库的全部授权（含主体与授权人）。
pub async fn list_grants(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    repository_id: Uuid,
) -> Result<Vec<CapabilityGrant>, GitCapabilityError> {
    let rows = sqlx::query(
        "SELECT g.repository_id, r.name AS repository_name, g.principal_id, \n                p.display_name AS principal_name, p.kind AS principal_kind, \n                g.capabilities, g.granted_by \n         FROM integration.git_capability_grant g \n         JOIN integration.project_repository r ON r.id = g.repository_id \n         JOIN core.principal p ON p.id = g.principal_id \n         WHERE g.tenant_id = $1 AND g.project_id = $2 AND g.repository_id = $3 \n         ORDER BY p.display_name, g.principal_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .fetch_all(pool)
    .await
    .map_err(|e| GitCapabilityError::Db(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| CapabilityGrant {
            repository_id: row.get("repository_id"),
            repository_name: row.get("repository_name"),
            principal_id: row.get("principal_id"),
            principal_name: row.get("principal_name"),
            principal_kind: row.get("principal_kind"),
            capabilities: row.get::<Vec<String>, _>("capabilities"),
            granted_by: row.get("granted_by"),
        })
        .collect())
}

/// 读某主体在某仓的**有效能力**（依赖闭包后；无授权 = 空集 = 全拒绝）。
pub async fn effective_capabilities_for(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    repository_id: Uuid,
    principal_id: Uuid,
) -> Result<std::collections::BTreeSet<GitCapability>, GitCapabilityError> {
    let raw: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT capabilities FROM integration.git_capability_grant \n         WHERE tenant_id = $1 AND project_id = $2 AND repository_id = $3 AND principal_id = $4",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| GitCapabilityError::Db(e.to_string()))?;
    let raw = raw.unwrap_or_default();
    let granted = pm_domain::parse_capabilities(&raw)
        .map_err(|unknown| GitCapabilityError::Validation(format!("unknown capability `{unknown}`")))?;
    Ok(pm_domain::effective_capabilities(&granted))
}

/// I3-C/D：单仓的能力范围快照（四层合流的产物）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RepoCapSnapshot {
    pub repository_id: Uuid,
    /// 叠加顺序：仓库策略（全部主体授权的并集）∩ Task 仓库范围（write_access=
    /// false → 仅 read）∩ 仓库状态（非 verified → 空）——最后过依赖闭包。
    pub effective: Vec<String>,
}

/// 构建 Run 的每仓能力快照（I3-C 消费 `git_capability_grant` 交集 + Task 范围）。
///
/// 层叠规则（I3 §2 四层，本函数覆盖第二/三层，第四层锚点见 `policy_snapshot_hash`）：
/// 1. 仓库策略 = 该仓**全部**授权行的能力并集（项目级策略，主体无关）；
/// 2. Task 仓库范围：`task_repository.write_access = false` → 只保留 `read`；
/// 3. 仓库状态：非 `verified` → 能力清空（§4.4）；
/// 4. 依赖闭包收口（`pm_domain::effective_capabilities`）。
pub async fn build_task_repo_caps_tx(
    tx: &mut PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<RepoCapSnapshot>, GitCapabilityError> {
    let rows: Vec<(Uuid, bool, String, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT tr.repository_id, tr.write_access, r.status, \
                (SELECT COALESCE(jsonb_agg(DISTINCT cap), '[]'::jsonb) \
                   FROM integration.git_capability_grant g, \
                        unnest(g.capabilities) AS cap \
                  WHERE g.tenant_id = r.tenant_id \
                    AND g.project_id = r.project_id \
                    AND g.repository_id = r.id) \
         FROM delivery.task_repository tr \
         JOIN integration.project_repository r ON r.id = tr.repository_id \
         WHERE tr.tenant_id = $1 AND tr.project_id = $2 AND tr.task_id = $3 \
         ORDER BY tr.repository_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(task_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| GitCapabilityError::Db(e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for (repository_id, write_access, repo_status, caps_json) in rows {
        let mut caps: Vec<GitCapability> = Vec::new();
        if repo_status == "verified" {
            let raw: Vec<String> = serde_json::from_value(caps_json.unwrap_or_default())
                .map_err(|e| GitCapabilityError::Validation(format!("grant caps: {e}")))?;
            caps = pm_domain::parse_capabilities(&raw).map_err(|unknown| {
                GitCapabilityError::Validation(format!("unknown capability `{unknown}`"))
            })?;
            if !write_access {
                // Task 范围收缩：只读仓——丢弃除 read 外的一切。
                caps.retain(|c| *c == GitCapability::Read);
            }
        }
        let effective = pm_domain::effective_capabilities(&caps);
        out.push(RepoCapSnapshot {
            repository_id,
            effective: effective.into_iter().map(|c| c.as_str().to_string()).collect(),
        });
    }
    Ok(out)
}

/// 快照 JSON（规范化形态：`[{repository_id, effective}]`，即写库内容）。
pub fn caps_snapshot_json(snaps: &[RepoCapSnapshot]) -> serde_json::Value {
    serde_json::to_value(snaps).unwrap_or_else(|_| serde_json::json!([]))
}

/// 快照的 SHA-256 内容寻址哈希（hex；I3 §5.4，写 `task_run.policy_snapshot_hash`）。
pub fn caps_snapshot_hash(snaps: &[RepoCapSnapshot]) -> String {
    crate::knowledge::content_hash(&caps_snapshot_json(snaps).to_string())
}

/// 整体替换某主体在某仓的授权集（PUT 语义；幂等）。
///
/// 单事务：行锁项目 → 服务端依赖校验（§3.3）→ upsert → 审计。
pub async fn set_grant(
    pool: &PgPool,
    actor: &crate::actor::AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    repository_id: Uuid,
    principal_id: Uuid,
    requested: Vec<GitCapability>,
) -> Result<Vec<GitCapability>, GitCapabilityError> {
    // §3.3：依赖是服务端校验规则，不是 UI 约束。
    let violations = pm_domain::prerequisite_violations(&requested);
    if let Some((cap, missing)) = violations.first() {
        return Err(GitCapabilityError::Validation(format!(
            "capability `{}` requires `{}` in the same grant set",
            cap.as_str(),
            missing.as_str()
        )));
    }
    let values: Vec<String> = requested.iter().map(|c| c.as_str().to_string()).collect();

    let mut tx = pool.begin().await.map_err(db_err)?;

    // 锁项目行（与 W13/I6-B 同向：project → grant 行），并校验可见范围。
    let project: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE id = $1 AND tenant_id = $2 FOR UPDATE")
            .bind(project_id)
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
    if project.is_none() {
        return Err(GitCapabilityError::NotFound("project not found"));
    }
    let repo: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM integration.project_repository \n         WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_err)?;
    if repo.is_none() {
        return Err(GitCapabilityError::NotFound("repository not found"));
    }
    let target_kind: Option<String> =
        sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
            .bind(principal_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
    if target_kind.is_none() {
        return Err(GitCapabilityError::NotFound("principal not found"));
    }

    let before: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT capabilities FROM integration.git_capability_grant \n         WHERE tenant_id = $1 AND project_id = $2 AND repository_id = $3 AND principal_id = $4 \n         FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .bind(principal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_err)?;

    sqlx::query(
        "INSERT INTO integration.git_capability_grant \n         (tenant_id, project_id, repository_id, principal_id, capabilities, granted_by) \n         VALUES ($1, $2, $3, $4, $5, $6) \n         ON CONFLICT (tenant_id, project_id, repository_id, principal_id) \n         DO UPDATE SET capabilities = EXCLUDED.capabilities, granted_by = EXCLUDED.granted_by",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .bind(principal_id)
    .bind(&values)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?;

    crate::pm_write::append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "git_capability.granted",
        "git_capability_grant",
        Some(repository_id),
        serde_json::json!({
            "principal_id": principal_id,
            "before": before,
            "after": values,
        }),
    )
    .await
    .map_err(db_err)?;

    tx.commit().await.map_err(db_err)?;
    Ok(requested)
}

/// 撤销某主体在某仓的全部授权（撤权即时生效）。
pub async fn revoke_grant(
    pool: &PgPool,
    actor: &crate::actor::AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    repository_id: Uuid,
    principal_id: Uuid,
) -> Result<(), GitCapabilityError> {
    let mut tx = pool.begin().await.map_err(db_err)?;
    let before: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT capabilities FROM integration.git_capability_grant \n         WHERE tenant_id = $1 AND project_id = $2 AND repository_id = $3 AND principal_id = $4 \n         FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .bind(principal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_err)?;
    sqlx::query(
        "DELETE FROM integration.git_capability_grant \n         WHERE tenant_id = $1 AND project_id = $2 AND repository_id = $3 AND principal_id = $4",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .bind(principal_id)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?;
    crate::pm_write::append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "git_capability.revoked",
        "git_capability_grant",
        Some(repository_id),
        serde_json::json!({ "principal_id": principal_id, "before": before, "after": [] }),
    )
    .await
    .map_err(db_err)?;
    tx.commit().await.map_err(db_err)?;
    Ok(())
}

fn db_err(err: sqlx::Error) -> GitCapabilityError {
    GitCapabilityError::Db(err.to_string())
}

/// 供判定层调用方使用的事务内读取（与 [`effective_capabilities_for`] 同口径）。
pub async fn effective_capabilities_in_tx(
    tx: &mut PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    repository_id: Uuid,
    principal_id: Uuid,
) -> Result<std::collections::BTreeSet<GitCapability>, GitCapabilityError> {
    let raw: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT capabilities FROM integration.git_capability_grant \n         WHERE tenant_id = $1 AND project_id = $2 AND repository_id = $3 AND principal_id = $4",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .bind(principal_id)
    .fetch_optional(tx)
    .await
    .map_err(|e| GitCapabilityError::Db(e.to_string()))?;
    let raw = raw.unwrap_or_default();
    let granted = pm_domain::parse_capabilities(&raw)
        .map_err(|unknown| GitCapabilityError::Validation(format!("unknown capability `{unknown}`")))?;
    Ok(pm_domain::effective_capabilities(&granted))
}
