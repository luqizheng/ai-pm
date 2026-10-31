//! Outbox 事件的生产 handler（spec §20.5 / §20.6）。
//!
//! 首版只注册一个有真实落点的事件：`TaskReady` → 为本任务创建一条
//! `delivery.task_run(status='queued')`，即「等待调度」的最小真实形态。
//! `queued` 只表示待执行，**不**把任务标记为 Running——执行器在 Slice D 领取。
//!
//! 幂等与并发：在**同一事务**里先对 `delivery.task` 行加锁（`FOR UPDATE`），再
//! 依次校验状态、执行资格与是否已有有效 Run，最后建 Run 并递增计数。行锁把同一
//! 任务的并发消费串行化；「已有 queued/running Run 则跳过」让至少一次投递下的重复
//! 消费坍缩为一次创建；任务已不在可调度态（过期事件）则确认消费但不产生副作用。

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::outbox::ClaimedOutboxEvent;
use crate::outbox_worker::OutboxHandler;

/// `TaskReady` 的消费 handler：为可调度的任务创建一条待执行 Run。
pub struct TaskReadyHandler {
    pool: PgPool,
}

impl TaskReadyHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn task_id_of(event: &ClaimedOutboxEvent) -> Option<Uuid> {
    event
        .payload
        .get("task_id")
        .and_then(|value| value.as_str())
        .and_then(|text| Uuid::parse_str(text).ok())
        .or_else(|| Uuid::parse_str(&event.aggregate_id).ok())
}

#[async_trait::async_trait]
impl OutboxHandler for TaskReadyHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)
            .ok_or_else(|| "TaskReady payload is missing a task_id".to_string())?;
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;

        let row = sqlx::query(
            "SELECT status, assignee_id FROM delivery.task \
             WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(task_id)
        .bind(event.project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let (status, assignee_id) = match row {
            Some(row) => (
                row.get::<String, _>("status"),
                row.get::<Option<Uuid>, _>("assignee_id"),
            ),
            // 任务已不存在：过期事件，确认消费。
            None => {
                tx.commit().await.map_err(|e| e.to_string())?;
                return Ok(());
            }
        };

        // 执行资格：只有仍处于可调度态（ready）且已指派 Agent 的任务才建 Run。
        // 状态已变化（过期事件）或未指派，则确认消费但不产生副作用。
        if status != "ready" || assignee_id.is_none() {
            tx.commit().await.map_err(|e| e.to_string())?;
            return Ok(());
        }

        // 幂等：已有有效 Run（queued/running）则不再新建。
        let active: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.task_run \
             WHERE task_id = $1 AND status IN ('queued', 'running')",
        )
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if active > 0 {
            tx.commit().await.map_err(|e| e.to_string())?;
            return Ok(());
        }

        let next_attempt: i32 = sqlx::query_scalar(
            "SELECT coalesce(max(attempt_no), 0) + 1 FROM delivery.task_run WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status) \
             VALUES ($1, $2, $3, 'queued')",
        )
        .bind(event.project_id)
        .bind(task_id)
        .bind(next_attempt)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query("UPDATE delivery.task SET attempt_count = attempt_count + 1 WHERE id = $1")
            .bind(task_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(())
    }
}