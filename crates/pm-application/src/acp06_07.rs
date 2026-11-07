//! ACP-06/07 · Checkpoint 登记 + Deadline 重新授权（0054）。
//!
//! 联合实施设计：`docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp06-07-joint-v1.md`
//! V1.0。经 `continuation::get_open_by_run` 明确接口接入 ACP-05，不吸收其职责。
//!
//! **ACP-06**：Checkpoint 正文永远在 Workspace（基线 §6，四不清理）；Platform
//! 仅登记元数据（两条通道：executor_report / worker_receipt，不判真实性）；
//! 无 Checkpoint 不阻止介入/继续（payload checkpoint=null → Agent 重建上下文）。
//! **ACP-07**：Deadline 重新授权 = 过期 Deadline 下启动新 Session 的**唯一合法
//! 凭证**——独立审计 + 独立有效期（专家 2.3 五约束）：`run.deadline_at` 零改动、
//! 交互有效期不动、回复不丢、未授权前禁自动启动；消费点 = recovery 续领门 +
//! F-1 continue guard 快照。

use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit_with_after;

// ───────────────────────────── ACP-06 Checkpoint ─────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum Acp067Error {
    #[error("database error: {0}")]
    Db(String),
    #[error("not found")]
    NotFound,
}

/// Checkpoint 登记行（§1.2 元数据集）。
#[derive(Debug, Clone, Serialize)]
pub struct TaskCheckpointRow {
    pub id: Uuid,
    pub run_id: Uuid,
    pub file_path: String,
    pub content_hash: String,
    pub file_size: i64,
    pub registered_via: String,
}

/// 执行端上报登记（`pm_report_checkpoint`；重复 (run_id, content_hash) 幂等
/// 返回原 id——ACP-478）。Worker 收割发现 → [`register_from_receipt`] 同函数。
#[allow(clippy::too_many_arguments)]
pub async fn register_checkpoint(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    interaction_id: Option<Uuid>,
    file_path: &str,
    content_hash: &str,
    file_size: i64,
    session_id: Option<&str>,
    registered_via: &str,
    actor: &AuditActor,
) -> Result<Uuid, Acp067Error> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| Acp067Error::Db(e.to_string()))?;
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO delivery.task_checkpoint \
           (tenant_id, project_id, task_id, run_id, interaction_id, file_path, \
            content_hash, file_size, session_id, registered_via) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, $5, \
                 $6, $7, $8, $9, $10) \
         ON CONFLICT (run_id, content_hash) DO NOTHING \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .bind(interaction_id)
    .bind(file_path)
    .bind(content_hash)
    .bind(file_size)
    .bind(session_id)
    .bind(registered_via)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| Acp067Error::Db(e.to_string()))?;
    let id = match inserted {
        Some(id) => id,
        None => {
            // 幂等：重复上报返回原 checkpoint id（ACP-478）。
            sqlx::query_scalar(
                "SELECT id FROM delivery.task_checkpoint \
                  WHERE run_id = $1 AND content_hash = $2",
            )
            .bind(run_id)
            .bind(content_hash)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| Acp067Error::Db(e.to_string()))?
        }
    };
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "checkpoint.registered",
        "task_checkpoint",
        Some(id),
        serde_json::json!({
            "run_id": run_id,
            "file_path": file_path,
            "content_hash": content_hash,
            "file_size": file_size,
            "registered_via": registered_via,
        }),
    )
    .await
    .map_err(|e| Acp067Error::Db(e.to_string()))?;
    tx.commit()
        .await
        .map_err(|e| Acp067Error::Db(e.to_string()))?;
    Ok(id)
}

/// Run 最新 Checkpoint 元数据（continue payload 引用；不含正文）。
pub async fn latest_for_run(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Option<(String, String)>, Acp067Error> {
    let row = sqlx::query(
        "SELECT file_path, content_hash FROM delivery.task_checkpoint \
          WHERE run_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| Acp067Error::Db(e.to_string()))?;
    Ok(row.map(|r| (r.get("file_path"), r.get("content_hash"))))
}

// ───────────────────── ACP-07 Deadline 重新授权（0054）─────────────────────

/// 授权行（独立审计 + 独立有效期）。
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionAuthorizationRow {
    pub id: Uuid,
    pub run_id: Uuid,
    pub authorized_by: Uuid,
    pub reason: String,
    pub new_deadline_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// 授予（REST Lead/Approver；不改 run.deadline_at——T3；审计独立行）。
pub async fn grant_execution_authorization(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    run_id: Uuid,
    interaction_id: Option<Uuid>,
    authorized_by: Uuid,
    reason: &str,
    new_deadline_at: chrono::DateTime<chrono::Utc>,
    actor: &AuditActor,
) -> Result<Uuid, Acp067Error> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| Acp067Error::Db(e.to_string()))?;
    let id = sqlx::query_scalar(
        "INSERT INTO delivery.task_run_execution_authorization \
           (tenant_id, project_id, run_id, interaction_id, authorized_by, reason, \
            new_deadline_at, expires_at) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, $5, $6, \
                 $7, $7) \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(run_id)
    .bind(interaction_id)
    .bind(authorized_by)
    .bind(reason)
    .bind(new_deadline_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| Acp067Error::Db(e.to_string()))?;
    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "run.execution_authorized",
        "task_run",
        Some(run_id),
        serde_json::json!({
            "authorization_id": id,
            "reason": reason,
            "new_deadline_at": new_deadline_at,
        }),
    )
    .await
    .map_err(|e| Acp067Error::Db(e.to_string()))?;
    tx.commit()
        .await
        .map_err(|e| Acp067Error::Db(e.to_string()))?;
    Ok(id)
}

/// 撤销（软撤；已消费的启动不追溯——启动事实在 worker_command ack）。
pub async fn revoke_execution_authorization(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    run_id: Uuid,
    authorization_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, Acp067Error> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| Acp067Error::Db(e.to_string()))?;
    let updated = sqlx::query(
        "UPDATE delivery.task_run_execution_authorization \
            SET revoked_at = now() \
          WHERE id = $1 AND run_id = $2 AND project_id = $3 AND tenant_id = $4 \
            AND revoked_at IS NULL \
         RETURNING id",
    )
    .bind(authorization_id)
    .bind(run_id)
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| Acp067Error::Db(e.to_string()))?;
    if let Some(row) = updated {
        let id: Uuid = row.get("id");
        append_audit_with_after(
            &mut tx,
            project_id,
            actor,
            "run.execution_authorization_revoked",
            "task_run",
            Some(run_id),
            serde_json::json!({ "authorization_id": id }),
        )
        .await
        .map_err(|e| Acp067Error::Db(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| Acp067Error::Db(e.to_string()))?;
        Ok(true)
    } else {
        tx.rollback()
            .await
            .map_err(|e| Acp067Error::Db(e.to_string()))?;
        Ok(false)
    }
}

/// 有效授权查询（§3.3 唯一消费判据）：未撤销 ∧ 未过期。deadline 过期场景的
/// 续领/启动仅凭此放行（未授权前禁自动启动）。
pub async fn find_valid_for_run(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    run_id: Uuid,
) -> Result<Option<ExecutionAuthorizationRow>, Acp067Error> {
    let row = sqlx::query(
        "SELECT id, run_id, authorized_by, reason, new_deadline_at, expires_at \
           FROM delivery.task_run_execution_authorization \
          WHERE tenant_id = $1 AND project_id = $2 AND run_id = $3 \
            AND revoked_at IS NULL AND expires_at > now() \
          ORDER BY granted_at DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| Acp067Error::Db(e.to_string()))?;
    Ok(row.map(|r| ExecutionAuthorizationRow {
        id: r.get("id"),
        run_id: r.get("run_id"),
        authorized_by: r.get("authorized_by"),
        reason: r.get("reason"),
        new_deadline_at: r.get("new_deadline_at"),
        expires_at: r.get("expires_at"),
    }))
}
