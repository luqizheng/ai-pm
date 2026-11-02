//! Outbox 运维服务层（T-061 §4）：死信列表 / 死信重放 / 事件状态监控。
//!
//! 这一层是**运维闭环**的应用层用例：既有死信查询与重放能力此前只躺在
//! 基础设施层（无任何 HTTP 入口），运维发现死信只能直连 DB。本模块把它们
//! 收敛成项目级可用的服务。
//!
//! **项目级隔离在 SQL 层保证**：所有查询与重放都以 `project_id` 为必填条件。
//! 跨项目事件与不存在事件**统一按「不存在」处理**——调用方拿不到「存在但
//! 无权」的信息，避免事件 ID 探测。

use uuid::Uuid;

/// 死信列表的一行（运维排障用）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeadLetterRow {
    pub id: Uuid,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 死信列表 + **项目全部死信总数**。
///
/// `total` 是**项目死信总数**（独立 COUNT，不带 limit），不是本次返回的条数——
/// 运维据此判断「一共积压了多少死信」才能决定是否继续排障。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeadLetterListing {
    pub items: Vec<DeadLetterRow>,
    pub total: i64,
}

/// 为什么重放被拒。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReplayError {
    /// 事件不存在，**或**不属于本项目（两者对外统一，避免事件 ID 探测）。
    #[error("outbox event not found")]
    NotFound,
    /// 事件属于本项目但不是死信（活事件仍在飞行中，重放会重复投递）。
    #[error("only dead-letter events can be replayed")]
    NotDeadLetter,
    #[error("outbox event replay failed: {0}")]
    Db(String),
}

/// 重放结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayOutcome {
    /// 已放回 `pending`，等待 worker 下一次领取。
    Replayed,
    NotFound,
    NotDeadLetter,
}

/// 事件状态监控快照。
///
/// **口径说明**：`failure_rate` 是**当前终态死信占比**（terminal_failure_ratio），
/// **不是历史失败率、也不是时间窗口失败率**——重放会把 `dead_letter → pending`
/// 并将 `attempts` 归零，历史失败事实会从当前状态统计中消失。下游（执行中心
/// 等）不得把它当作系统历史可靠性指标。
#[derive(Debug, Clone, serde::Serialize)]
pub struct OutboxStats {
    /// 按 `status` 分组的行数（四值全覆盖，缺者为 0）。
    pub by_status: StatusCounts,
    /// 按 `event_type` × `status` 的交叉计数；**只列出有行的类型**。
    pub by_event_type: Vec<EventTypeCounts>,
    /// `dead_letter / (delivered + dead_letter)`；分母为 0 时取 `0.0`。
    pub failure_rate: f64,
    /// `pending` + `processing` 两类里**最老**那条的年龄（秒）；无积压时为 `0`。
    pub oldest_pending_age_secs: i64,
    /// 死信行数（与 `by_status.dead_letter` 冗余，让运维一眼看到告警值）。
    pub dead_letter_count: i64,
}

/// 四种状态的计数（缺者为 0）。
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StatusCounts {
    pub pending: i64,
    pub processing: i64,
    pub delivered: i64,
    pub dead_letter: i64,
}

/// 单个事件类型的四状态计数。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EventTypeCounts {
    pub event_type: String,
    pub pending: i64,
    pub processing: i64,
    pub delivered: i64,
    pub dead_letter: i64,
}

/// 列出某项目的死信（最新在前）+ 项目全部死信总数。
///
/// 两条查询（分页 + 独立 COUNT）**带相同的 `project_id` 条件**。
pub async fn dead_letters(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    limit: i64,
) -> Result<DeadLetterListing, sqlx::Error> {
    let items = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            String,
            i32,
            Option<String>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT id, event_type, aggregate_type, aggregate_id, attempts, last_error, created_at \
         FROM integration.outbox_event \
         WHERE status = 'dead_letter' AND project_id = $1 \
         ORDER BY created_at DESC \
         LIMIT $2",
    )
    .bind(project_id)
    .bind(limit)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(
        |(id, event_type, aggregate_type, aggregate_id, attempts, last_error, created_at)| {
            DeadLetterRow {
                id,
                event_type,
                aggregate_type,
                aggregate_id,
                attempts,
                last_error,
                created_at,
            }
        },
    )
    .collect();
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event \
         WHERE status = 'dead_letter' AND project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    Ok(DeadLetterListing { items, total })
}

/// 重放一条死信（单条，一条一次）。
///
/// **单条原子 UPDATE 同时校验 `id` / `project_id` / `status`**——API 层的
/// 项目可见性检查不足以证明底层只操作本项目的事件（调用方可构造别项目的
/// `event_id`）。0 行时再用同样带 `project_id` 的查询区分「本项目内但不是
/// 死信」与「不存在 / 跨项目」；跨项目的行视同不存在，**不回读其任何字段**。
///
/// 重放**不保证消费成功**：只是把事件放回队列，真正的消费由 worker 的下一次
/// 领取完成。重放归零 `attempts`——若处理器的 bug 没修，重放会再死信一次，
/// 这是期望行为（不是无限重试，是「再给 5 次」）。
pub async fn replay(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    event_id: Uuid,
) -> Result<ReplayOutcome, ReplayError> {
    let updated: Option<Uuid> = sqlx::query_scalar(
        "UPDATE integration.outbox_event \
         SET status = 'pending', attempts = 0, next_attempt_at = now(), \
             lease_owner = NULL, locked_until = NULL \
         WHERE id = $1 AND project_id = $2 AND status = 'dead_letter' \
         RETURNING id",
    )
    .bind(event_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ReplayError::Db(e.to_string()))?;
    if updated.is_some() {
        return Ok(ReplayOutcome::Replayed);
    }
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM integration.outbox_event WHERE id = $1 AND project_id = $2",
    )
    .bind(event_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ReplayError::Db(e.to_string()))?;
    Ok(match status {
        None => ReplayOutcome::NotFound,
        Some(_) => ReplayOutcome::NotDeadLetter,
    })
}

/// 事件状态监控快照。
///
/// 口径：
/// - `failure_rate` 只衡量「已走完生命周期的事件里失败的比例」——`pending`
///   是**还没处理**不是**处理失败**，算进去会把刚产生的正常事件计为失败；
/// - `oldest_pending_age_secs` 取 `pending` 与 `processing` 的**共同最老值**——
///   `processing` 行可能是卡住的（worker 崩溃后租约过期的行会留在那里直到
///   被重新领取），两者一起才能暴露「worker 死了没人捞」。
pub async fn stats(pool: &sqlx::PgPool, project_id: Uuid) -> Result<OutboxStats, sqlx::Error> {
    let status_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM integration.outbox_event \
          WHERE project_id = $1 GROUP BY status",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let mut by_status = StatusCounts::default();
    for (status, count) in status_rows {
        match status.as_str() {
            "pending" => by_status.pending = count,
            "processing" => by_status.processing = count,
            "delivered" => by_status.delivered = count,
            "dead_letter" => by_status.dead_letter = count,
            _ => {}
        }
    }

    let type_rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT event_type, status, count(*) FROM integration.outbox_event \
          WHERE project_id = $1 GROUP BY event_type, status ORDER BY event_type",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let mut by_event_type: Vec<EventTypeCounts> = Vec::new();
    for (event_type, status, count) in type_rows {
        let entry = match by_event_type
            .iter_mut()
            .find(|entry| entry.event_type == event_type)
        {
            Some(entry) => entry,
            None => {
                by_event_type.push(EventTypeCounts {
                    event_type: event_type.clone(),
                    pending: 0,
                    processing: 0,
                    delivered: 0,
                    dead_letter: 0,
                });
                by_event_type.last_mut().expect("just pushed")
            }
        };
        match status.as_str() {
            "pending" => entry.pending = count,
            "processing" => entry.processing = count,
            "delivered" => entry.delivered = count,
            "dead_letter" => entry.dead_letter = count,
            _ => {}
        }
    }

    let terminal = by_status.delivered + by_status.dead_letter;
    let failure_rate = if terminal > 0 {
        by_status.dead_letter as f64 / terminal as f64
    } else {
        0.0
    };

    let oldest: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT min(created_at) FROM integration.outbox_event \
          WHERE project_id = $1 AND status IN ('pending', 'processing')",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    let oldest_pending_age_secs = oldest
        .map(|at| (chrono::Utc::now() - at).num_seconds().max(0))
        .unwrap_or(0);

    Ok(OutboxStats {
        dead_letter_count: by_status.dead_letter,
        by_status,
        by_event_type,
        failure_rate,
        oldest_pending_age_secs,
    })
}
