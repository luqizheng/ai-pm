//! S05 执行监控最小事件账（roadmap §9 最小方案）：`delivery.task_run_event`
//! 的唯一写入入口。
//!
//! 立场（迁移 0013 头注同源）：
//! - **事件账不是执行记录重建**：执行事实仍在 `delivery.task_run` /
//!   `task_repository_commit` / manifest 视图；本表只补「库内无 run 级过程
//!   时刻」的监控数据面。
//! - **只记无专表承载的生命周期时刻**：Run 领取/启动/阶段/终态与 Worker 命令
//!   生命周期。interaction / continuation / response_delivery 已有专表，监控
//!   面联表取数，本表不重复落账。
//! - **不做心跳级事件**：心跳/进程存活已有 run 行列（T4 四信号分立）。
//! - **review Run 不落账**：D5 平台内确定性 Reviewer（无外部 Agent、无租约
//!   世代），其状态直接读 run 行；事件面只覆盖 delivery Run 与 Worker 命令。
//! - **同事务幂等写入**：与主操作同事务提交（失败整体回滚，不降级吞错）；
//!   幂等键 `UNIQUE (run_id, event_type, lease_generation, dedupe_tag)` +
//!   `ON CONFLICT DO NOTHING`。
//!
//! 事件类型不是状态机（无转移语义），因此常量落在本模块而不进 pm-domain；
//! DB 侧 CHECK 是第二道防线。

use sqlx::PgConnection;
use uuid::Uuid;

/// 与 `0013_task_run_event.sql` 的 CHECK 白名单严格同步。
pub const EVENT_TYPES: &[&str] = &[
    "run_claimed",
    "run_started",
    "run_stage_changed",
    "run_finished",
    "command_issued",
    "command_delivered",
    "command_acked",
    "command_failed",
];

/// 事件类型常量（调用点用静态字符串，避免魔法值散落）。
pub mod event_type {
    pub const RUN_CLAIMED: &str = "run_claimed";
    pub const RUN_STARTED: &str = "run_started";
    pub const RUN_STAGE_CHANGED: &str = "run_stage_changed";
    pub const RUN_FINISHED: &str = "run_finished";
    pub const COMMAND_ISSUED: &str = "command_issued";
    pub const COMMAND_DELIVERED: &str = "command_delivered";
    pub const COMMAND_ACKED: &str = "command_acked";
    pub const COMMAND_FAILED: &str = "command_failed";
}

/// 一条 run 事件的载荷。`generation` 传 `None` 时从 run 行快照
/// `lease_generation`（终态/清理路径常无 generation 在手）。
///
/// `dedupe_tag` 语义：`run_stage_changed`=stage 名（或 `requeued` 等回归
/// 标记）；`command_*=命令 id`；`run_claimed` / `run_started` / `run_finished`
/// 用空串（同 generation 内天然至多一次）。
#[derive(Debug, Clone)]
pub struct RunEvent<'a> {
    pub event_type: &'a str,
    pub generation: Option<i32>,
    pub dedupe_tag: &'a str,
    pub detail: Option<serde_json::Value>,
}

/// 记录一条 run 事件（幂等）。`tenant/project/task/client` 一律从 run 行快照，
/// 不信任调用点传参——所有打点都发生在 run 行已存在的事务内。
/// 返回实际插入行数（1=新落账，0=幂等去重）。
pub async fn record_run_event(
    conn: &mut PgConnection,
    run_id: Uuid,
    event: RunEvent<'_>,
) -> Result<u64, sqlx::Error> {
    debug_assert!(
        EVENT_TYPES.contains(&event.event_type),
        "unknown run event type: {}",
        event.event_type
    );
    debug_assert!(
        event.dedupe_tag.len() <= 200,
        "dedupe_tag exceeds 200 chars"
    );
    sqlx::query(
        "INSERT INTO delivery.task_run_event \
            (tenant_id, project_id, task_id, run_id, client_id, event_type, \
             lease_generation, dedupe_tag, detail) \
         SELECT tenant_id, project_id, task_id, id, lease_client_id, $2, \
                COALESCE($3, lease_generation), $4, $5 \
           FROM delivery.task_run WHERE id = $1 \
         ON CONFLICT (run_id, event_type, lease_generation, dedupe_tag) DO NOTHING",
    )
    .bind(run_id)
    .bind(event.event_type)
    .bind(event.generation)
    .bind(event.dedupe_tag)
    .bind(event.detail)
    .execute(conn)
    .await
    .map(|r| r.rows_affected())
}
