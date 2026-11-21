//! 仓库访问判定与 Run 授权快照（S01 · I3 V1.1 简化裁决，迁移 `0033`）。
//!
//! 简化后的模型（`docs/ui/smark-pm-s01-i3-…-v1.md` §0.0.2）：
//!
//! ```text
//! 可访问仓库 = Task 关联仓库（delivery.task_repository）∩ 仓库状态 = verified
//! 访问档位   = read_write（可读即可写——写操作不再按类型分级）
//! ```
//!
//! 执行期判定 = **快照（上界：本次 Run 定格的可访问仓库清单）**
//!            ∩ **现状（仓库仍 verified ∧ Project 未归档）**。
//!
//! 已撤下（V1.0 方案，2026-11-04 用户简化裁决）：
//! 逐主体/逐仓授权表（`integration.git_capability_grant`，迁移 0033 已 DROP）、
//! 六能力枚举与依赖闭包、显式撤权动作。
//!
//! 保留的访问边界：租户/项目隔离、Task 仓库范围、仓库状态、
//! Run 授权快照（审计追溯）、Project 归档门禁、Run 取消门禁。

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use pm_domain::repo_access::RepoAccess;

/// 单仓访问快照（一行 = 一个本次 Run 可访问的仓库）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RepoAccessSnapshot {
    pub repository_id: Uuid,
    /// 访问档位（当前唯一档位 `read_write`）。
    pub access: String,
}

/// 访问判定失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoAccessError {
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("{0}")]
    Validation(String),
    #[error("database error: {0}")]
    Db(String),
}

impl From<RepoAccessError> for sqlx::Error {
    fn from(err: RepoAccessError) -> Self {
        match err {
            RepoAccessError::Db(message) => sqlx::Error::Protocol(message),
            other => sqlx::Error::Protocol(other.to_string()),
        }
    }
}

/// 构建 Run 的可访问仓库快照（Task 范围 ∩ 仓库状态 verified）。
///
/// 规则（简化裁决 §0.0.2）：
/// 1. 仓库必须在 Task 的关联范围内（`delivery.task_repository`）——不在范围 = 不可访问；
/// 2. 仓库状态必须为 `verified`——未验证/已归档的仓库不可访问；
/// 3. 档位恒为 `read_write`（不区分操作类型）。
///
/// 不在范围内的仓库**不出现**在快照里（缺项即拒绝，不做"降级只读"）。
pub async fn build_task_repo_access_tx(
    tx: &mut PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<RepoAccessSnapshot>, RepoAccessError> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT tr.repository_id \
         FROM delivery.task_repository tr \
         JOIN integration.project_repository r \
           ON r.tenant_id = tr.tenant_id \
          AND r.project_id = tr.project_id \
          AND r.id = tr.repository_id \
         WHERE tr.tenant_id = $1 AND tr.project_id = $2 AND tr.task_id = $3 \
           AND r.status = 'verified' \
         ORDER BY tr.repository_id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(task_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| RepoAccessError::Db(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(repository_id,)| RepoAccessSnapshot {
            repository_id,
            access: RepoAccess::ReadWrite.as_str().to_string(),
        })
        .collect())
}

/// 快照 JSON（规范化形态：`[{repository_id, access}]`，即写库内容）。
pub fn access_snapshot_json(snaps: &[RepoAccessSnapshot]) -> serde_json::Value {
    serde_json::to_value(snaps).unwrap_or_else(|_| serde_json::json!([]))
}

/// 快照的 SHA-256 内容寻址哈希（hex；写 `task_run.policy_snapshot_hash`）。
pub fn access_snapshot_hash(snaps: &[RepoAccessSnapshot]) -> String {
    crate::knowledge::content_hash(&access_snapshot_json(snaps).to_string())
}

/// 执行期强校验——某次 Git 操作是否被**本次 Run 的定格快照**允许。
///
/// 规则：
/// - 快照是**不可变事实**：执行期只读快照，不实时改快照；
/// - 快照缺失（领取早于 0033 部署或快照写失败）→ **保守拒绝**（deny-all）；
/// - 快照里没有该仓 / 该仓档位不足 → 拒绝。
pub fn snapshot_allows(
    snapshot: Option<&serde_json::Value>,
    repository_id: Uuid,
    required: RepoAccess,
) -> bool {
    let Some(snapshot) = snapshot else {
        return false;
    };
    let empty = Vec::new();
    let entries = snapshot
        .get("repo_access")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    entries.iter().any(|entry| {
        entry
            .get("repository_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            == Some(repository_id)
            && entry.get("access").and_then(|v| v.as_str()) == Some(required.as_str())
    })
}

/// 执行期强校验的**实时**版本——快照允许，但现状已变化 → 拒绝。
///
/// 「历史 Run 保留执行时授权快照，但不得凭历史快照继续执行**当前已不可用**的操作」
/// （总体计划 §4.2 / 简化裁决 §0.0.4）：快照是**上界**，现状是**现状**——两者取交集。
/// 现状由两类状态变化构成：
/// - 仓库被撤出 Task 范围或状态不再 `verified`；
/// - Project 已归档（I1 门禁）。
pub async fn execution_allows(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    repository_id: Uuid,
    snapshot: Option<&serde_json::Value>,
    required: RepoAccess,
) -> Result<bool, RepoAccessError> {
    if !snapshot_allows(snapshot, repository_id, required) {
        return Ok(false);
    }
    // 现状 1：Project 归档 → 拒绝（I1）。
    match crate::project_gate::project_activity(pool, project_id).await {
        Ok(crate::project_gate::ProjectActivity::Active) => {}
        Ok(crate::project_gate::ProjectActivity::Closed) => return Ok(false),
        Err(crate::project_gate::GateError::NotFound) => return Ok(false),
        Err(err) => return Err(RepoAccessError::Db(err.to_string())),
    }
    // 现状 2：仓库仍属本项目且仍 verified → 允许。
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM integration.project_repository \
         WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| RepoAccessError::Db(e.to_string()))?;
    Ok(matches!(status.as_deref(), Some("verified")))
}
