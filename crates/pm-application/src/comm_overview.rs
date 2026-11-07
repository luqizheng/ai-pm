//! ACP 通信面可观测聚合（审计 F-5）：shielded Run / 未消费交互 / 命令积压。
//!
//! 仓内零 metrics 基建（grep 证实）——V1 采用 house style 的聚合函数 +
//! worker 周期结构化日志承载指标（`interaction_shielded_runs` /
//! `interaction_unconsumed_total` / `worker_command_pending_total` /
//! `worker_command_pending_age_seconds`，字段名与设计稿一致，接入
//! Prometheus/OTLP 时原样映射）。纯 SQL 聚合 + 纯结构，单测可测。

use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommOverview {
    /// 按 guard 值分维的屏蔽 Run 数（V1.1 §4.1；不含 none）。
    pub interaction_shielded_runs: i64,
    /// RESPONDED 未消费稳态计数（N-6 合法稳态的告警面）。
    pub interaction_unconsumed_total: i64,
    /// 开放介入链数（chain_open）。
    pub interaction_open_chains: i64,
    /// 未回执命令数（issued + delivered）。
    pub worker_command_pending_total: i64,
    /// 最老未回执命令的年龄（秒；无则 0）。
    pub worker_command_pending_age_seconds: i64,
}

/// 全局（跨项目）聚合——worker 周期日志面；项目级过滤随控制台需求另加。
pub async fn load_comm_overview(pool: &PgPool) -> Result<CommOverview, sqlx::Error> {
    let (interaction_shielded_runs, interaction_unconsumed_total, interaction_open_chains): (
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM delivery.task_run WHERE scheduling_shield <> 'none'), \
            (SELECT count(*) FROM delivery.task_interaction WHERE status = 'responded'), \
            (SELECT count(*) FROM delivery.task_interaction WHERE chain_open)",
    )
    .fetch_one(pool)
    .await?;

    let (worker_command_pending_total, oldest): (i64, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT count(*), min(issued_at) FROM delivery.worker_command \
          WHERE status IN ('issued', 'delivered')",
    )
    .fetch_one(pool)
    .await?;
    let worker_command_pending_age_seconds = oldest
        .map(|o| (Utc::now() - o).num_seconds().max(0))
        .unwrap_or(0);

    Ok(CommOverview {
        interaction_shielded_runs,
        interaction_unconsumed_total,
        interaction_open_chains,
        worker_command_pending_total,
        worker_command_pending_age_seconds,
    })
}
