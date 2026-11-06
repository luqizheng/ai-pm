//! Project 级知识写入协调（T-060 / v1.3，设计稿 §5.8）。
//!
//! **Agent 执行并行 ≠ 知识写入并行**：允许多个 Agent 在不同任务 / 仓上并行
//! 执行，但**同一 Project 的知识库写入必须串行**。
//!
//! 协调靠 PostgreSQL **会话级 advisory lock**——跨实例天然有效，实例崩溃时
//! 连接断开即自动释放（不留死锁）。锁粒度 = `project_id`：不同 Project 完全
//! 并行，无全局写锁。
//!
//! 写入五步（§5.8.3）：
//!
//! 1. 取锁（超时 → 409 `KNOWLEDGE_PROJECT_BUSY`）；
//! 2. `base_revision` 校验（与当前 head 不一致 → 409，不产生 Revision）；
//! 3. 幂等预占（业务事务内、`operation_id` 唯一键冲突即重放）；
//! 4. 业务事务提交（正文写 Git + 修订落库 + 回填账本）；
//! 5. 同步重建索引 → 释放锁。

use std::time::{Duration, Instant};

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::knowledge::KnowledgeError;

/// 等待 project 写锁的默认上限（毫秒）。
pub const DEFAULT_WRITE_LOCK_TIMEOUT_MS: u64 = 10_000;

/// 持锁期间独占该 Project 的写入权。
///
/// 连接**不归还池**——会话级 advisory lock 随连接存活，把连接放回池会让锁
/// 被别的请求继承。释放时显式 `pg_advisory_unlock` 后 `detach()`；异常路径
/// 同样 `detach()`，会话结束由 PG 自动释放。
pub struct ProjectWriteGuard {
    conn: Option<PgConnection>,
    project_id: Uuid,
    wait_ms: u64,
}

impl ProjectWriteGuard {
    /// 取锁成功后的等待时长（毫秒）——写日志用。
    pub fn wait_ms(&self) -> u64 {
        self.wait_ms
    }

    pub fn project_id(&self) -> Uuid {
        self.project_id
    }

    /// 显式释放锁并丢弃连接。
    pub async fn release(mut self) {
        if let Some(conn) = self.conn.as_mut() {
            let _ = sqlx::query("SELECT pg_advisory_unlock(hashtextextended($1::text, 0))")
                .bind(lock_key(self.project_id))
                .execute(&mut *conn)
                .await;
        }
        // 不放回池：会话锁会跟着连接被继承。
        self.conn.take();
    }
}

impl Drop for ProjectWriteGuard {
    fn drop(&mut self) {
        // 兜底：连接被丢弃后 PG 自动释放会话锁（§5.8.2「不留死锁」）。
        self.conn.take();
    }
}

fn lock_key(project_id: Uuid) -> String {
    format!("ai.project_knowledge_write:{project_id}")
}

/// 取 project 级写锁。超时 → `KnowledgeError::ProjectWriteBusy`。
pub async fn acquire(
    pool: &PgPool,
    project_id: Uuid,
    timeout: Duration,
) -> Result<ProjectWriteGuard, KnowledgeError> {
    let started = Instant::now();
    let key = lock_key(project_id);
    loop {
        let conn = pool.acquire().await.map_err(KnowledgeError::Db)?;
        let mut conn = conn.detach();
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1::text, 0))")
            .bind(&key)
            .fetch_one(&mut conn)
            .await
            .map_err(KnowledgeError::Db)?;
        if acquired {
            tracing::info!(
                project_id = %project_id,
                wait_ms = started.elapsed().as_millis() as u64,
                "knowledge_write_locked"
            );
            return Ok(ProjectWriteGuard {
                conn: Some(conn),
                project_id,
                wait_ms: started.elapsed().as_millis() as u64,
            });
        }
        // 没抢到：连接丢弃（不能放回池，否则带着未持有的锁入池），
        // 短暂退避后重试。
        drop(conn);
        if started.elapsed() >= timeout {
            tracing::warn!(
                project_id = %project_id,
                wait_ms = started.elapsed().as_millis() as u64,
                "knowledge_write_busy"
            );
            return Err(KnowledgeError::ProjectWriteBusy(format!(
                "project {project_id} write lock timed out after {} ms",
                started.elapsed().as_millis()
            )));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 写命令的四种类型（幂等账本的 `command` 取值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteCommand {
    CreateDocument,
    SubmitProposal,
    ConfirmRedline,
    VetoRevision,
    /// S02-I2：需求修订 submit 时的投影绑定（bind_projection）。
    BindRequirementProjection,
}

impl WriteCommand {
    pub fn as_str(&self) -> &'static str {
        match self {
            WriteCommand::CreateDocument => "create_document",
            WriteCommand::SubmitProposal => "submit_proposal",
            WriteCommand::ConfirmRedline => "confirm_redline",
            WriteCommand::VetoRevision => "veto_revision",
            WriteCommand::BindRequirementProjection => "bind_requirement_projection",
        }
    }
}

/// 幂等预占的结果。
pub enum OperationClaim {
    /// 首次占用——调用方继续执行业务写入。
    Fresh { operation_id: Uuid },
    /// 重放——既有账本行；调用方**不得**再写 Git / 再产生 Revision。
    Replayed {
        proposal_id: Option<Uuid>,
        revision_id: Option<Uuid>,
    },
}

/// 在业务事务内**预占** `operation_id`。
///
/// 唯一键冲突即代表重放：调用方读到既有行并返回既有结果，不二次发布。
pub async fn claim_operation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    operation_id: &str,
    command: WriteCommand,
) -> Result<OperationClaim, KnowledgeError> {
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO ai.knowledge_write_operation \
         (tenant_id, project_id, operation_id, command) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3) \
         ON CONFLICT (project_id, operation_id) DO NOTHING RETURNING id",
    )
    .bind(project_id)
    .bind(operation_id)
    .bind(command.as_str())
    .fetch_optional(&mut **tx)
    .await?;

    match inserted {
        Some(id) => Ok(OperationClaim::Fresh { operation_id: id }),
        None => {
            let row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
                "SELECT proposal_id, revision_id FROM ai.knowledge_write_operation \
                  WHERE project_id = $1 AND operation_id = $2",
            )
            .bind(project_id)
            .bind(operation_id)
            .fetch_optional(&mut **tx)
            .await?;
            let (proposal_id, revision_id) = row.ok_or_else(|| {
                KnowledgeError::Conflict(format!(
                    "write operation {operation_id} conflicted but its ledger row is missing"
                ))
            })?;
            tracing::info!(
                project_id = %project_id,
                operation_id = %operation_id,
                revision_id = ?revision_id,
                "knowledge_write_replayed"
            );
            Ok(OperationClaim::Replayed {
                proposal_id,
                revision_id,
            })
        }
    }
}

/// 回填账本行产出的提案 / 修订 id。
pub async fn complete_operation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    operation_id: &str,
    proposal_id: Option<Uuid>,
    revision_id: Option<Uuid>,
) -> Result<(), KnowledgeError> {
    sqlx::query(
        "UPDATE ai.knowledge_write_operation \
            SET proposal_id = COALESCE($3, proposal_id), \
                revision_id = COALESCE($4, revision_id) \
          WHERE project_id = $1 AND operation_id = $2",
    )
    .bind(project_id)
    .bind(operation_id)
    .bind(proposal_id)
    .bind(revision_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 读账本行（重放时取既有结果）。
pub async fn find_operation(
    pool: &PgPool,
    project_id: Uuid,
    operation_id: &str,
) -> Result<Option<(Option<Uuid>, Option<Uuid>)>, KnowledgeError> {
    Ok(sqlx::query_as(
        "SELECT proposal_id, revision_id FROM ai.knowledge_write_operation \
          WHERE project_id = $1 AND operation_id = $2",
    )
    .bind(project_id)
    .bind(operation_id)
    .fetch_optional(pool)
    .await?)
}

/// 当前 head 修订（该文档最新 published 修订）。
pub async fn current_head(
    pool: &PgPool,
    project_id: Uuid,
    doc_key: &str,
) -> Result<Option<Uuid>, KnowledgeError> {
    Ok(sqlx::query_scalar(
        "SELECT r.id FROM ai.project_document d \
           JOIN LATERAL ( \
               SELECT id FROM ai.document_revision \
                WHERE document_id = d.id AND status = 'published' \
                ORDER BY revision_no DESC LIMIT 1 \
           ) r ON true \
          WHERE d.project_id = $1 AND d.doc_key = $2",
    )
    .bind(project_id)
    .bind(doc_key)
    .fetch_optional(pool)
    .await?)
}

/// `base_revision` 校验（§5.8.3 第 2 步）。
///
/// - 请求**携带** `base_revision_id` 时，它必须等于该文档当前 head（最新
///   published 修订）；否则 → 409 `KNOWLEDGE_REVISION_CONFLICT`，不产生 Revision；
/// - 请求**未携带** `base_revision_id`（`None`）时不做校验（调用方放弃乐观并发，
///   走「后写覆盖」语义）；
/// - 新建文档必须携带 `None`（没有既有文档可作基线）。
pub fn check_base_revision(
    requested: Option<Uuid>,
    current_head: Option<Uuid>,
) -> Result<(), KnowledgeError> {
    match requested {
        None => Ok(()),
        Some(base) if Some(base) == current_head => Ok(()),
        Some(base) => Err(KnowledgeError::RevisionConflict(format!(
            "base revision {base} does not match the current head {:?}",
            current_head
        ))),
    }
}
