//! Outbox 写入端口（task state machine spec §20.2 / §20.6）。
//!
//! 业务事务内写入「待发布事件」，让「状态已变、事件却没写」不可表示：`append`
//! 只接受 `&mut Transaction`，事件行随调用方事务一起提交或回滚。投递为至少一次
//! （§20.6.3），故写入按 `idempotency_key` 幂等去重——重复的语义事件在写入时
//! 坍缩为同一行，消费侧 handler 亦须幂等。
//!
//! 端口定义在应用层、由 Postgres 直写适配（[`PgOutboxAppend`]）：应用层不依赖
//! 基础设施层的 outbox 模块，却仍与业务状态变更共用**同一个**事务，不引入第二
//! 套事务（§20.6.1）。

use async_trait::async_trait;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// 一个待写入的 outbox 事件，落在调用方业务事务里。
pub struct OutboxEvent<'a> {
    pub project_id: Uuid,
    pub event_type: &'a str,
    pub aggregate_type: &'a str,
    pub aggregate_id: &'a str,
    pub payload: serde_json::Value,
    /// 语义幂等键：相同键的重复事件在写入时坍缩为同一行。
    pub idempotency_key: &'a str,
}

/// 在同一业务事务内写入 outbox 事件的端口。
#[async_trait]
pub trait OutboxAppend: Send + Sync {
    async fn append<'a>(
        &self,
        tx: &mut Transaction<'a, Postgres>,
        event: OutboxEvent<'_>,
    ) -> Result<Uuid, sqlx::Error>;
}

/// Postgres 直写适配器：在既有事务里 INSERT，冲突键幂等复用既有行。
pub struct PgOutboxAppend;

#[async_trait]
impl OutboxAppend for PgOutboxAppend {
    async fn append<'a>(
        &self,
        tx: &mut Transaction<'a, Postgres>,
        event: OutboxEvent<'_>,
    ) -> Result<Uuid, sqlx::Error> {
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO integration.outbox_event \
             (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (idempotency_key) DO NOTHING \
             RETURNING id",
        )
        .bind(event.project_id)
        .bind(event.event_type)
        .bind(event.aggregate_type)
        .bind(event.aggregate_id)
        .bind(event.payload)
        .bind(event.idempotency_key)
        .fetch_optional(&mut **tx)
        .await?;
        match inserted {
            Some(id) => Ok(id),
            None => {
                sqlx::query_scalar(
                    "SELECT id FROM integration.outbox_event WHERE idempotency_key = $1",
                )
                .bind(event.idempotency_key)
                .fetch_one(&mut **tx)
                .await
            }
        }
    }
}
/// 某事件类型当前是否允许产生（§20.6：对暂不支持的事件用功能开关关闭，不产生
/// 无 handler 可消费的悬空事件）。
///
/// `AI_PM_OUTBOX_EVENTS` 为逗号分隔的允许清单；未设置时默认只开两类已有明确触发
/// 语义的事件（`TaskReady` / `DeliverySubmitted`）。设成空串即关闭全部事件的产生。
pub fn event_is_enabled(event_type: &str) -> bool {
    match std::env::var("AI_PM_OUTBOX_EVENTS") {
        Ok(value) => value.split(',').map(str::trim).any(|item| item == event_type),
        Err(_) => matches!(event_type, "TaskReady" | "DeliverySubmitted"),
    }
}