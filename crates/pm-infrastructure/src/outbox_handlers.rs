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
        // 状态已变化（过期事件）或未指派，则确认消费但不产生副作用（留痕，不静默）。
        if status != "ready" || assignee_id.is_none() {
            let reason = if status != "ready" {
                "task_not_ready"
            } else {
                "no_assignee"
            };
            tracing::info!(%task_id, reason, "TaskReady consumed without scheduling");
            tx.commit().await.map_err(|e| e.to_string())?;
            return Ok(());
        }

        // 任务级幂等（L0）：已有活跃 Run 则不再新建。三处口径必须一致：
        // 状态集合复用 run_dispatch 的单一真源（ACTIVE_RUN_STATUSES_SQL），
        // 且**必须带 `run_kind = 'delivery'` 谓词**——收窄后的 uq_task_run_active
        // 只约束 delivery，若计数不带谓词，活跃 review run 会挡住 delivery run。
        let active: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM delivery.task_run \
             WHERE task_id = $1 AND {} AND status IN ({})",
            pm_application::run_dispatch::DELIVERY_RUN_KIND_SQL,
            pm_application::run_dispatch::ACTIVE_RUN_STATUSES_SQL
        ))
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if active > 0 {
            tracing::info!(%task_id, reason = "active_run_exists", "TaskReady consumed: active run exists");
            tx.commit().await.map_err(|e| e.to_string())?;
            return Ok(());
        }

        // L1 事件级幂等 + B3-6 共享封装：把创建它的事件 id 绑定进 Run（此处已持有
        // task 行锁，满足 B2 的 INSERT 顺序要求）。
        match pm_application::run_dispatch::insert_task_run(
            &mut tx,
            event.project_id,
            task_id,
            Some(event.id),
        )
        .await
        {
            Ok(_) => {}
            // L2 冲突降级：uq_task_run_source_event（同一事件已建 Run，如旧事件 replay
            // 而旧 Run 已终态）或 uq_task_run_active（并发先到者已建）冲突 → 幂等成功
            // 确认消费，**不进死信**。冲突时整个事务回滚，计数不会半更新。
            Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
                tracing::info!(
                    %task_id,
                    event_id = %event.id,
                    "TaskReady consumed: idempotent conflict, run already exists"
                );
                tx.rollback().await.map_err(|e| e.to_string())?;
                return Ok(());
            }
            Err(e) => return Err(e.to_string()),
        }

        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// `DeliverySubmitted` 的消费 handler（D5 拍①）：推进 `submitted→reviewing` 并为
/// 本次交付创建一条 queued review run。
///
/// 幂等由 D5 应用层判据承担（存在锚定 `(task_id, input_manifest_hash)` 的任意
/// review run → 幂等返回），因此重复消费坍缩为一次创建；`23505`（并发先到者已建，
/// `uq_task_review_active` / 锚定冲突）同样降级为幂等成功，**不进死信**。
pub struct DeliverySubmittedHandler {
    pool: PgPool,
}

impl DeliverySubmittedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for DeliverySubmittedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)
            .ok_or_else(|| "DeliverySubmitted payload is missing a task_id".to_string())?;
        match pm_application::review::enqueue_review(&self.pool, event.project_id, task_id).await {
            Ok(outcome) => {
                tracing::info!(%task_id, ?outcome, "DeliverySubmitted consumed");
                Ok(())
            }
            Err(pm_application::review::ReviewError::Db(sqlx::Error::Database(db)))
                if db.code().as_deref() == Some("23505") =>
            {
                tracing::info!(
                    %task_id,
                    "DeliverySubmitted consumed: idempotent conflict, review run already exists"
                );
                Ok(())
            }
            Err(error) => Err(error.to_string()),
        }
    }
}