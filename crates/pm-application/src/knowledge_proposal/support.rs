use super::*;
use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};
use crate::knowledge_quality::{self, GateFailure, GateInput, QualityGateResult};
use serde::Serialize;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

pub(crate) async fn require_human(pool: &PgPool, principal_id: Uuid) -> Result<(), KnowledgeError> {
    let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
        .bind(principal_id)
        .fetch_optional(pool)
        .await?;
    match kind.as_deref() {
        Some("human") => Ok(()),
        // DEF-P0-03：授权拒绝不是状态冲突——落 Forbidden（HTTP 403），
        // 与需求域 `requirement::require_human`（PmWriteError::Forbidden）同口径。
        Some(other) => Err(KnowledgeError::Forbidden(format!(
            "principal {principal_id} is `{other}`, a human confirmation is required"
        ))),
        None => Err(KnowledgeError::NotFound),
    }
}

/// 知识域审计行。`actor` 的 `principal_id` 是执行主体（人工 = 真人 / MCP = 项目
/// `ai_pm`）；真人三列与设备归因来自认证上下文，缺省为 NULL（T-088）。
pub(crate) async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    actor: &crate::actor::AuditActor,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    after_data: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'knowledge_v2', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(after_data)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 供索引层复用的质量门结果序列化。
pub fn gates_as_json(result: &QualityGateResult) -> serde_json::Value {
    serde_json::to_value(result).unwrap_or_default()
}
