//! Slice D1：Run 调度与领取（`docs/ai-pm-slice-d-execution-pipeline-v1.md` §五 / §十一）。
//!
//! 本模块是 D1 的唯一调度落点：复用 `delivery.task_run` 作队列，在**单事务**内
//! 完成「选候选 Run → 锁任务行 → 可领取性判定 → 状态推进 → 多仓基线快照 → 审计」。
//!
//! 硬不变量（B2）：全局加锁顺序为 `run(FOR UPDATE OF r SKIP LOCKED) → task(FOR UPDATE)`，
//! 且**任何新建 Run 的 INSERT 都发生在取得 task 行锁之后**。唯一索引冲突会等待而非
//! 立即失败，把新 Run 的插入放在 task 锁之后是避免死锁的前提。
//!
//! 状态语义（S4）：Run 的 `claimed` 表示「已领取、执行端尚未启动」；Task 的
//! `claimed` 是 13 态状态机里的独立状态。两者用不同常量表达，不混用字符串字面量。

#![allow(unused_imports)]

pub(crate) use sqlx::{PgPool, Postgres, Row, Transaction};
pub(crate) use uuid::Uuid;

pub(crate) use pm_domain::TaskStatus;

pub(crate) use crate::command_executor::{
    apply_task_transition, system_context, ExecutionContext, TransitionFacts,
};
pub(crate) use crate::policy_composition::{
    EffectivePolicy, PolicyLayer, PolicyLayers, POLICY_SCHEMA,
};

/// Run 生命周期状态（S4：独立于 [`TaskStatus`] 的枚举，避免同名 `claimed` 混用）。
pub mod run_status {
    pub const QUEUED: &str = "queued";
    pub const CLAIMED: &str = "claimed";
    pub const RUNNING: &str = "running";
    pub const SUCCEEDED: &str = "succeeded";
    pub const FAILED: &str = "failed";
    pub const TIMED_OUT: &str = "timed_out";
    pub const CANCELLED: &str = "cancelled";
}

/// 执行阶段（Q9 / B3）：`dispatch` = 尚未启动执行器；`started` = 执行器已启动。
pub mod run_stage {
    pub const DISPATCH: &str = "dispatch";
    pub const STARTED: &str = "started";
}

/// 活跃 Run 集合（B1 / I1 统一定义，必须与 `uq_task_run_active` 的部分谓词一致）。
pub const ACTIVE_RUN_STATUSES: [&str; 3] =
    [run_status::QUEUED, run_status::CLAIMED, run_status::RUNNING];

/// `SQL IN (...)` 片段，供各查询复用，杜绝各处手写不一致的集合。
pub const ACTIVE_RUN_STATUSES_SQL: &str = "'queued', 'claimed', 'running'";

/// Run 种类（D5 Q-D5-1）：`delivery` = D1/D3 执行 Run；`review` = D5 Reviewer Run。
///
/// 两条恢复链的 `run_kind` 各自恒定、互不越界（D5 §5.4 不变量 4）。活跃 Run
/// 的判定必须带上本谓词——收窄后的 `uq_task_run_active` 只约束 delivery，
/// 若计数不带谓词，一个活跃 review run 会挡住 delivery run 的创建与领取。
pub mod run_kind {
    pub const DELIVERY: &str = "delivery";
    pub const REVIEW: &str = "review";
}

/// `run_kind = 'delivery'` 谓词片段（与 [`ACTIVE_RUN_STATUSES_SQL`] 配套使用）。
pub const DELIVERY_RUN_KIND_SQL: &str = "run_kind = 'delivery'";

/// Reviewer Run 的活跃状态集合（D5 §2.1 `uq_task_review_active` 的谓词：
/// review run 不走 `claimed`——`executor_kind='platform'`，领取即 `running`）。
pub const ACTIVE_REVIEW_STATUSES: [&str; 2] = [run_status::QUEUED, run_status::RUNNING];

/// `SQL IN (...)` 片段，与 [`ACTIVE_REVIEW_STATUSES`] 一致。
pub const ACTIVE_REVIEW_STATUSES_SQL: &str = "'queued', 'running'";

mod claim;
mod lease;
mod policy;
mod port;
mod transition;

pub use claim::*;
pub use lease::*;
pub use policy::*;
pub use port::*;
pub use transition::*;

#[derive(Debug, thiserror::Error)]
pub enum RunDispatchError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("state transition refused: {0}")]
    Transition(String),
    #[error("no actor available for audit")]
    NoActor,
    /// Model A：任务不存在或不在调用方项目内（不泄露跨项目存在性）。
    #[error("task not found in this project")]
    TaskNotFound,
    /// Model A：任务当前状态不是 `ready`（附当前状态）。
    #[error("task is not ready (current status: {0})")]
    TaskNotReady(String),
    /// Model A：该任务已有活跃 Run（queued/claimed/running），不允许再领。
    #[error("the task already has an active run")]
    RunAlreadyClaimed,
    /// Model A：没有可领取的 Run（TaskReadyHandler 尚未创建）。
    #[error("no runnable run for the task (wait for TaskReady)")]
    NoRunnableRun,
    #[error("invalid claim option: {0}")]
    InvalidOption(String),
    #[error("lease token generation failed")]
    Random,
}

/// 调度模式（R4 / Q8）：**只控制新 Run 的领取**，不控制租约健康检查与过期回收。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchMode {
    /// 默认：不领取新 Run；回收/健康检查仍持续。
    Off,
    /// 领取全流程后立即按 B3-1 释放回 `queued`（`release_reason='probe_release'`）。
    Probe,
    /// 领取后把 Run 交给已注册的 `RunExecutor`（D3 接入 Codex 后启用）。
    On,
}

impl DispatchMode {
    pub fn from_env() -> Self {
        match std::env::var("AI_PM_RUN_DISPATCH").ok().as_deref() {
            Some("on") => Self::On,
            Some("probe") => Self::Probe,
            _ => Self::Off,
        }
    }

    /// 是否允许**领取**新 Run。回收不受此开关约束（R4）。
    pub fn claims(self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// 调度参数（读 `AI_PM_*` 环境变量，不硬编码）。
#[derive(Debug, Clone, Copy)]
pub struct RunDispatchConfig {
    pub lease_secs: i64,
    pub backoff_cap_secs: i64,
    pub max_run_attempts: i32,
    pub max_infrastructure_attempts: i32,
    /// Run 超时（D2 契约 `run.timeout_seconds` 的库内来源，领取时写入）。
    pub timeout_secs: i32,
    /// T-032 T3：`task.timeout_seconds` 缺省时的绝对执行截止兜底（秒）。
    /// `deadline_at = 首次 claim 时刻 + COALESCE(timeout_seconds, 本值)`。
    pub deadline_default_secs: i64,
    /// T-032 T4：Agent 心跳失效阈值（秒）——进程存活但心跳停滞超阈值即终止。
    pub agent_heartbeat_stale_secs: i64,
    /// T-032 T4：无进展阈值（秒）——`last_progress_at` 停滞超阈值即终止。
    pub progress_stale_secs: i64,
}

impl RunDispatchConfig {
    pub fn from_env() -> Self {
        fn num(key: &str, default: i64) -> i64 {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }
        Self {
            lease_secs: num("AI_PM_RUN_LEASE_SECS", 300),
            backoff_cap_secs: num("AI_PM_RUN_BACKOFF_CAP_SECS", 300),
            max_run_attempts: num("AI_PM_MAX_RUN_ATTEMPTS", 3) as i32,
            max_infrastructure_attempts: num("AI_PM_MAX_INFRA_ATTEMPTS", 5) as i32,
            timeout_secs: num("AI_PM_RUN_TIMEOUT_SECS", 1800) as i32,
            deadline_default_secs: num("AI_PM_RUN_DEADLINE_DEFAULT_SECS", 3600),
            agent_heartbeat_stale_secs: num("AI_PM_RUN_AGENT_HEARTBEAT_STALE_SECS", 900),
            progress_stale_secs: num("AI_PM_RUN_PROGRESS_STALE_SECS", 3600),
        }
    }

    /// 心跳间隔（契约 `run.lease.heartbeat_interval_secs` 的配置来源）。
    pub fn heartbeat_interval_secs() -> i64 {
        std::env::var("AI_PM_RUN_HEARTBEAT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60)
    }
}

/// 一次成功创建的 Run（共享封装返回值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatedRun {
    pub run_id: Uuid,
    pub attempt_no: i32,
}
