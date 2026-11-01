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

use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use pm_domain::TaskStatus;

use crate::command_executor::{
    apply_task_transition, system_context, ExecutionContext, TransitionFacts,
};
use crate::policy_composition::{EffectivePolicy, PolicyLayer, PolicyLayers, POLICY_SCHEMA};

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
pub const ACTIVE_RUN_STATUSES: [&str; 3] = [
    run_status::QUEUED,
    run_status::CLAIMED,
    run_status::RUNNING,
];

/// `SQL IN (...)` 片段，供各查询复用，杜绝各处手写不一致的集合。
pub const ACTIVE_RUN_STATUSES_SQL: &str = "'queued', 'claimed', 'running'";

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

/// **共享 Run 创建封装（B3-6）**：创建一条 `queued` Run、递增 `attempt_no` 与
/// `task.attempt_count`。`TaskReadyHandler` 与租约恢复路径**必须**复用本函数，
/// 不得各自实现一套 attempt 计算。
///
/// 前置条件（B2）：调用者必须已在**同一事务**内对 `delivery.task` 对应行加
/// `FOR UPDATE`。新 Run 的 INSERT 必须在取得 task 行锁之后执行。
///
/// `source_event_id`（L1 事件级幂等，`docs/ai-pm-taskready-idempotency-v1.md` §五）：
/// 由 outbox 事件触发时传入事件行 id，使「一个事件至多创建一条 Run」由
/// `uq_task_run_source_event` 兜底；租约恢复路径（非事件触发）传 `None`。
pub async fn insert_task_run(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    source_event_id: Option<Uuid>,
) -> Result<CreatedRun, sqlx::Error> {
    let attempt_no: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(attempt_no), 0) + 1 FROM delivery.task_run WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;

    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, stage, next_dispatch_at, source_event_id) \
         VALUES ($1, $2, $3, 'queued', 'dispatch', now(), $4) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .bind(source_event_id)
    .fetch_one(&mut **tx)
    .await?;

    // Q9：attempt_count 是 Task 生命周期内 Run 创建总数。
    sqlx::query("UPDATE delivery.task SET attempt_count = attempt_count + 1 WHERE id = $1")
        .bind(task_id)
        .execute(&mut **tx)
        .await?;

    Ok(CreatedRun { run_id, attempt_no })
}

/// R6：固定的工作区路径算法。`repos/<safe-name>-<repository_id>`——`repository_id`
/// 后缀保证唯一与可复现；仓库改名不改变既有映射；`safe_name` 只保留 `[a-z0-9-]`，
/// 从字符层杜绝非法字符与路径穿越（`..`、`/`、`\`）。
pub fn workspace_path_for(repository_id: Uuid, name: &str) -> String {
    let mut safe = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            safe.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_' || ch == ' ') && !safe.ends_with('-') {
            safe.push('-');
        }
    }
    let safe = safe.trim_matches('-');
    let safe = if safe.is_empty() { "repo" } else { safe };
    format!("repos/{safe}-{repository_id}")
}

// ---------------------------------------------------------------------------
// RunExecutor 端口（R7：端口在 pm-application，实现留在 D3 的 pm-infrastructure）
// ---------------------------------------------------------------------------

/// 一次可执行的 Run：平台已锁定任务、基线与策略快照。
#[derive(Debug, Clone)]
pub struct ExecutableRun {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub attempt_no: i32,
    pub timeout_seconds: i32,
    /// D1 产出：按仓库锁定的基线（含 `workspace_path`）。
    pub repositories: Vec<RunRepositoryBaseline>,
}

/// 每仓锁定的基线（与 `delivery.task_run_repository` 对应）。
#[derive(Debug, Clone)]
pub struct RunRepositoryBaseline {
    pub repository_id: Uuid,
    pub base_ref: String,
    pub base_commit: String,
    pub write_access: bool,
    pub workspace_path: String,
}

/// 执行端会话（D3 填充）。
#[derive(Debug, Clone)]
pub struct ExecutorSession {
    pub session: String,
}

/// 一次轮询得到的阶段进度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorProgress {
    Pending,
    Running,
    Done,
}

/// 终止信号的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminateReason {
    Cancelled,
    TimedOut,
    LeaseReclaimed,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("executor does not support this operation")]
    Unsupported,
    #[error("executor failure: {0}")]
    Other(String),
}

/// 执行器端口（D1 只定义生命周期契约，D3 再实现真实执行器）。
#[async_trait::async_trait]
pub trait RunExecutor: Send + Sync {
    fn kind(&self) -> &'static str;
    async fn start(&self, run: &ExecutableRun) -> Result<ExecutorSession, ExecutorError>;
    async fn poll(&self, session: &ExecutorSession) -> Result<ExecutorProgress, ExecutorError>;
    async fn terminate(
        &self,
        session: &ExecutorSession,
        reason: TerminateReason,
    ) -> Result<(), ExecutorError>;
}

/// Noop 执行器（R7）：只实现接口，**绝不伪造执行成功**——`start` 直接返回
/// [`ExecutorError::Unsupported`]，因此 D1 永远不会把 Run 推进到 `running`/`succeeded`。
pub struct NoopExecutor;

#[async_trait::async_trait]
impl RunExecutor for NoopExecutor {
    fn kind(&self) -> &'static str {
        "noop"
    }

    async fn start(&self, _run: &ExecutableRun) -> Result<ExecutorSession, ExecutorError> {
        Err(ExecutorError::Unsupported)
    }

    async fn poll(&self, _session: &ExecutorSession) -> Result<ExecutorProgress, ExecutorError> {
        Err(ExecutorError::Unsupported)
    }

    async fn terminate(
        &self,
        _session: &ExecutorSession,
        _reason: TerminateReason,
    ) -> Result<(), ExecutorError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 领取 / 回收
// ---------------------------------------------------------------------------

/// 一次 `claim_once` 的结果类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimKind {
    /// 正常领取（`queued → claimed`）或恢复重领同一 Run（B3-1）。
    Claimed,
    /// 已启动执行器的旧 Run 失联，已隔离并新建 Run（B3-2）。
    Reclaimed,
    /// 条件不满足：Run 退避回 `queued`。
    Skipped,
    /// 任务已终态/被取代/不存在：Run 落 `cancelled`。
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimOutcome {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub kind: ClaimKind,
    /// Model A 签发的一次性租约明文；Model B 与未真正领取的分支都为 `None`。
    pub lease_token: Option<String>,
}

/// 租约持有者（A 裁决）：Model B 是 worker 标识，Model A 是 MCP 客户端。
///
/// **`lease_owner` 不是凭证**：它只用于展示与 fencing 的辅助归因，真正的凭证
/// 是 `lease_token`（见 `docs/ai-pm-slice-d2-mcp-context-v1.md` §三 A 的关键推论）。
#[derive(Debug, Clone, Copy)]
enum LeaseOwner<'a> {
    Worker(&'a str),
    Mcp { client_name: &'a str },
}

/// 领取路径的全部分叉点（A-2：复用同一事务模板，只以参数表达差异）。
#[derive(Debug, Clone, Copy)]
struct ClaimOptions<'a> {
    lease_owner: LeaseOwner<'a>,
    /// Model A 必填：把候选限定在调用方项目内。
    project_filter: Option<Uuid>,
    /// Model A 必填：只领指定任务。
    task_filter: Option<Uuid>,
    /// Model A 恒 `false`（A-1：执行端显式请求不受 `AI_PM_RUN_DISPATCH` 约束）。
    gate_by_dispatch_mode: bool,
    mode: DispatchMode,
    /// 只有 Model A 签发 `lease_token`。
    issue_token: bool,
    client_name: Option<&'a str>,
    client_version: Option<&'a str>,
}

/// Model A（执行端显式领取）成功后的返回：一次领取即可拿到的全部凭据。
#[derive(Debug, Clone)]
pub struct ClaimedTaskRun {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub attempt_no: i32,
    /// 32 字节随机数的小写 hex（64 字符），**只返回这一次**。
    pub lease_token: String,
    pub lease_expires_at: chrono::DateTime<chrono::Utc>,
    pub git_branch: String,
    pub stage: String,
}

/// 执行策略读取端口（G5）：策略读取失败必须**拒绝领取**，不得默认放行。
/// 默认实现等价于此前的字面量 `true`，测试可注入失败实现（验收 C12）。
pub trait ExecutionPolicy: Send + Sync {
    fn execution_enabled(&self) -> Result<bool, String>;
}

/// 生产默认策略：可执行（与 D1 快照中的字面量语义一致）。
pub struct DefaultExecutionPolicy;

impl ExecutionPolicy for DefaultExecutionPolicy {
    fn execution_enabled(&self) -> Result<bool, String> {
        Ok(true)
    }
}

/// 领取（或回收）**恰好一条**可执行的 Run；无候选返回 `Ok(None)`。
///
/// 语义要点：
/// - 候选 = `queued` 且到期，或 `claimed` 且租约过期。**领取与回收是同一条路径**
///   （复用 outbox 的 reclaim 范式），因此不需要独立的 reaper（需求 10）。
/// - B3-1：`executor_session IS NULL` 的租约过期 → 重领**同一** Run，不新建、不记失败。
/// - B3-2：`executor_session IS NOT NULL` 的失联 → 旧 Run 落 `timed_out` 并记录终止
///   结果，再新建 Run（`attempt_no+1`、`supersedes_run_id` 指向旧 Run）。
pub async fn claim_once(
    pool: &PgPool,
    worker_id: &str,
    cfg: &RunDispatchConfig,
    mode: DispatchMode,
) -> Result<Option<ClaimOutcome>, RunDispatchError> {
    claim_once_with_policy(pool, worker_id, cfg, mode, &DefaultExecutionPolicy).await
}

/// 同 [`claim_once`]，但可注入执行策略读取实现（G5 / 验收 C12）。
pub async fn claim_once_with_policy(
    pool: &PgPool,
    worker_id: &str,
    cfg: &RunDispatchConfig,
    mode: DispatchMode,
    policy: &dyn ExecutionPolicy,
) -> Result<Option<ClaimOutcome>, RunDispatchError> {
    claim_core(
        pool,
        cfg,
        &ClaimOptions {
            lease_owner: LeaseOwner::Worker(worker_id),
            project_filter: None,
            task_filter: None,
            gate_by_dispatch_mode: true,
            mode,
            issue_token: false,
            client_name: None,
            client_version: None,
        },
        policy,
    )
    .await
}

/// **Model A（A 裁决）：执行端显式领取指定任务**。
///
/// 不受 `AI_PM_RUN_DISPATCH` 开关约束（A-1）——该开关只门控 worker 的无人值守领取。
/// 成功时一次性签发 `lease_token`（明文只在这里返回，库内只存 sha256 小写 hex）。
///
/// 前置条件（§6.1）：任务不存在/跨项目 → [`RunDispatchError::TaskNotFound`]；
/// 没有可领取的 Run（TaskReadyHandler 尚未创建）→ [`RunDispatchError::NoRunnableRun`]；
/// 已有其他活跃 Run → [`RunDispatchError::RunAlreadyClaimed`]；
/// 任务非 `ready` → [`RunDispatchError::TaskNotReady`]。
pub async fn claim_task_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    client_name: &str,
    client_version: Option<&str>,
    cfg: &RunDispatchConfig,
) -> Result<ClaimedTaskRun, RunDispatchError> {
    claim_task_run_with_policy(
        pool,
        project_id,
        task_id,
        client_name,
        client_version,
        cfg,
        &DefaultExecutionPolicy,
    )
    .await
}

/// 同 [`claim_task_run`]，但可注入执行策略读取实现（G5 / 验收 C12）。
#[allow(clippy::too_many_arguments)]
pub async fn claim_task_run_with_policy(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    client_name: &str,
    client_version: Option<&str>,
    cfg: &RunDispatchConfig,
    policy: &dyn ExecutionPolicy,
) -> Result<ClaimedTaskRun, RunDispatchError> {
    let outcome = claim_core(
        pool,
        cfg,
        &ClaimOptions {
            lease_owner: LeaseOwner::Mcp { client_name },
            project_filter: Some(project_id),
            task_filter: Some(task_id),
            gate_by_dispatch_mode: false,
            mode: DispatchMode::On,
            issue_token: true,
            client_name: Some(client_name),
            client_version,
        },
        policy,
    )
    .await?;
    let outcome = outcome.ok_or(RunDispatchError::NoRunnableRun)?;
    let lease_token = outcome.lease_token.ok_or(RunDispatchError::NoRunnableRun)?;
    let (attempt_no, lease_expires_at, stage, git_branch): (
        i32,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT r.attempt_no, r.lease_expires_at, r.stage, t.git_branch \
           FROM delivery.task_run r JOIN delivery.task t ON t.id = r.task_id \
          WHERE r.id = $1 AND r.project_id = $2",
    )
    .bind(outcome.run_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    Ok(ClaimedTaskRun {
        run_id: outcome.run_id,
        task_id: outcome.task_id,
        attempt_no,
        lease_token,
        lease_expires_at: lease_expires_at
            .ok_or_else(|| RunDispatchError::InvalidOption("lease_expires_at missing".into()))?,
        git_branch: git_branch
            .ok_or_else(|| RunDispatchError::InvalidOption("git_branch missing".into()))?,
        stage: stage.unwrap_or_else(|| run_stage::DISPATCH.to_string()),
    })
}

/// A/B 两入口的**唯一事务模板**（A-2）：候选筛选、租约签发、开关门控与审计
/// payload 都由 [`ClaimOptions`] 表达，禁止复制事务体。
async fn claim_core(
    pool: &PgPool,
    cfg: &RunDispatchConfig,
    opts: &ClaimOptions<'_>,
    policy: &dyn ExecutionPolicy,
) -> Result<Option<ClaimOutcome>, RunDispatchError> {
    let mut tx = pool.begin().await?;

    // ① 选候选 Run（不阻塞其他 Worker）。顺序：先锁 run。
    let candidate = if let Some(task_id) = opts.task_filter {
        let project_id = opts
            .project_filter
            .ok_or_else(|| RunDispatchError::InvalidOption("Model A requires a project scope".into()))?;
        sqlx::query(
            "SELECT r.id, r.task_id, r.project_id, r.status, r.dispatch_attempts, \
                    r.executor_session, r.attempt_no \
               FROM delivery.task_run r \
              WHERE r.task_id = $1 AND r.project_id = $2 \
                AND ((r.status = 'queued'  AND r.next_dispatch_at <= now()) \
                  OR (r.status = 'claimed' AND r.lease_expires_at < now())) \
              ORDER BY r.attempt_no DESC, r.created_at \
              FOR UPDATE OF r SKIP LOCKED \
              LIMIT 1",
        )
        .bind(task_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
    } else {
        sqlx::query(
            "SELECT r.id, r.task_id, r.project_id, r.status, r.dispatch_attempts, \
                    r.executor_session, r.attempt_no \
               FROM delivery.task_run r \
              WHERE (r.status = 'queued'  AND r.next_dispatch_at <= now()) \
                 OR (r.status = 'claimed' AND r.lease_expires_at < now()) \
              ORDER BY r.next_dispatch_at, r.created_at \
              FOR UPDATE OF r SKIP LOCKED \
              LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?
    };
    let Some(candidate) = candidate else {
        if let Some(task_id) = opts.task_filter {
            // Model A：区分「任务不存在」「已有活跃 Run」「等 TaskReady」三种前置失败。
            let project_id = opts
                .project_filter
                .ok_or_else(|| RunDispatchError::InvalidOption("Model A requires a project scope".into()))?;
            let task_exists: Option<String> = sqlx::query_scalar(
                "SELECT status FROM delivery.task WHERE id = $1 AND project_id = $2",
            )
            .bind(task_id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
            if task_exists.is_none() {
                tx.rollback().await?;
                return Err(RunDispatchError::TaskNotFound);
            }
            let active: i64 = sqlx::query_scalar(concat!(
                "SELECT count(*) FROM delivery.task_run \
                  WHERE task_id = $1 AND status IN (",
                "'queued', 'claimed', 'running'",
                ")"
            ))
            .bind(task_id)
            .fetch_one(&mut *tx)
            .await?;
            tx.rollback().await?;
            return Err(if active > 0 {
                RunDispatchError::RunAlreadyClaimed
            } else {
                RunDispatchError::NoRunnableRun
            });
        }
        tx.commit().await?;
        return Ok(None);
    };
    let mut run_id: Uuid = candidate.get("id");
    let task_id: Uuid = candidate.get("task_id");
    let project_id: Uuid = candidate.get("project_id");
    let run_state: String = candidate.get("status");
    let dispatch_attempts: i32 = candidate.get("dispatch_attempts");
    let executor_session: Option<String> = candidate.get("executor_session");

    // ② 锁任务行（顺序固定：run → task）。B2：新 Run 的 INSERT 必须在此之后。
    let task = sqlx::query(
        "SELECT status, revision, retry_exhausted, task_type FROM delivery.task \
          WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(task) = task else {
        cancel_run(&mut tx, run_id, "task_missing").await?;
        tx.commit().await?;
        if opts.task_filter.is_some() {
            return Err(RunDispatchError::TaskNotFound);
        }
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Cancelled,
            lease_token: None,
        }));
    };
    let mut task_state: String = task.get("status");
    let retry_exhausted: bool = task.get("retry_exhausted");
    let task_type: Option<String> = task.get("task_type");

    // ③ 任务已终态 / 被取代 → Run 是死记录，落 cancelled，不动任务。
    let terminal_or_gone = TaskStatus::from_str(&task_state)
        .map(|s| s.is_terminal())
        .unwrap_or(true);
    if terminal_or_gone {
        cancel_run(&mut tx, run_id, "task_terminal").await?;
        tx.commit().await?;
        if opts.task_filter.is_some() {
            return Err(RunDispatchError::TaskNotReady(task_state));
        }
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Cancelled,
            lease_token: None,
        }));
    }

    // R1：构造系统执行身份（审计 actor 必须存在）；所有 Task 状态变更经统一
    // 受守卫服务 apply_task_transition。
    let ctx = system_context(&mut tx, project_id)
        .await
        .map_err(|e| RunDispatchError::Transition(e.to_string()))?;

    // ④ 回收分支：候选原本是 `claimed` 且租约已过期。
    if run_state == run_status::CLAIMED {
        if executor_session.is_none() {
            // B3-1：执行器尚未启动 → 恢复**同一** Run（重置租约回队列），不新建、不记失败。
            // 任务仍在 claimed：把它退回 ready 以便重新领取（同一 Run）。
            if task_state == "claimed" {
                apply_transition(&mut tx, task_id, TaskStatus::Claimed, TaskStatus::Ready, &ctx).await?;
                task_state = "ready".to_string();
            }
            requeue_for_redispatch(&mut tx, run_id).await?;
            audit_run(
                &mut tx,
                project_id,
                "reclaim_task_run",
                run_id,
                serde_json::json!({ "reason": "lease_expired_unstarted", "run_id": run_id }),
            )
            .await?;
            // Model B 到此为止（下一轮再领取）；Model A 在同一事务内继续完成领取。
            if opts.task_filter.is_none() {
                tx.commit().await?;
                return Ok(Some(ClaimOutcome {
                    run_id,
                    task_id,
                    kind: ClaimKind::Claimed,
                    lease_token: None,
                }));
            }
        } else {
            // B3-2：执行器已启动且失联 → 隔离旧实例、记录终止结果，再新建 Run。
            isolate_started_run(&mut tx, run_id, cfg).await?;
            let superseded_run_id = run_id;
            // 任务先退回 ready，统一走「正常领取」路径重建。
            if task_state == "claimed" || task_state == "running" {
                let from = TaskStatus::from_str(&task_state).unwrap_or(TaskStatus::Claimed);
                apply_transition(&mut tx, task_id, from, TaskStatus::Ready, &ctx).await?;
                task_state = "ready".to_string();
            }
            let created = insert_task_run(&mut tx, project_id, task_id, None).await?;
            sqlx::query("UPDATE delivery.task_run SET supersedes_run_id = $2 WHERE id = $1")
                .bind(created.run_id)
                .bind(superseded_run_id)
                .execute(&mut *tx)
                .await?;
            audit_run(
                &mut tx,
                project_id,
                "reclaim_task_run",
                superseded_run_id,
                serde_json::json!({
                    "reason": "lease_expired_started",
                    "new_run_id": created.run_id,
                    "attempt_no": created.attempt_no,
                }),
            )
            .await?;
            if opts.task_filter.is_none() {
                tx.commit().await?;
                return Ok(Some(ClaimOutcome {
                    run_id: created.run_id,
                    task_id,
                    kind: ClaimKind::Reclaimed,
                    lease_token: None,
                }));
            }
            run_id = created.run_id;
        }
    }

    // ⑤ 正常领取：任务必须严格为 ready。R4：调度关闭时不领取新 Run
    // （回收分支已在上方执行，不受开关影响）。Model A 恒不受开关门控（A-1）。
    if opts.gate_by_dispatch_mode && !opts.mode.claims() {
        tx.commit().await?;
        return Ok(None);
    }
    if task_state != "ready" {
        // Model A：显式请求遇到非 ready 任务是前置条件失败，**不得**把 Run
        // 推入退避（那会改变无人值守队列的节奏）。
        if opts.task_filter.is_some() {
            tx.rollback().await?;
            return Err(RunDispatchError::TaskNotReady(task_state));
        }
        requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, "task_not_ready").await?;
        audit_run(
            &mut tx,
            project_id,
            "skip_task_run",
            run_id,
            serde_json::json!({ "release_reason": "task_not_ready", "dispatch_attempts": dispatch_attempts + 1 }),
        )
        .await?;
        tx.commit().await?;
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Skipped,
            lease_token: None,
        }));
    }

    // ⑥ 执行策略门（G5）：读取失败不得默认放行，且拒绝路径审计要留下失败快照。
    match policy.execution_enabled() {
        Ok(true) => {}
        Ok(false) => {
            requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, "policy_disabled").await?;
            audit_run(
                &mut tx,
                project_id,
                "skip_task_run",
                run_id,
                serde_json::json!({
                    "release_reason": "policy_disabled",
                    "policy_snapshot": { "execution_enabled": false },
                    "dispatch_attempts": dispatch_attempts + 1,
                }),
            )
            .await?;
            tx.commit().await?;
            return Ok(Some(ClaimOutcome {
                run_id,
                task_id,
                kind: ClaimKind::Skipped,
                lease_token: None,
            }));
        }
        Err(error) => {
            requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, "policy_unavailable")
                .await?;
            audit_run(
                &mut tx,
                project_id,
                "skip_task_run",
                run_id,
                serde_json::json!({
                    "release_reason": "policy_unavailable",
                    "policy_snapshot": { "execution_enabled": false, "error": error },
                    "dispatch_attempts": dispatch_attempts + 1,
                }),
            )
            .await?;
            tx.commit().await?;
            return Ok(Some(ClaimOutcome {
                run_id,
                task_id,
                kind: ClaimKind::Skipped,
                lease_token: None,
            }));
        }
    }

    // ⑦ 可领取性判定（全部通过才领取）。
    let assessment =
        claimability_assessment(&mut tx, project_id, task_id, retry_exhausted, task_type.as_deref())
            .await?;
    if let Some(reason) = assessment.deny {
        if opts.task_filter.is_some() {
            tx.rollback().await?;
            return Err(RunDispatchError::TaskNotReady(format!("denied: {reason}")));
        }
        requeue_with_backoff(&mut tx, run_id, dispatch_attempts, cfg, reason).await?;
        audit_run(
            &mut tx,
            project_id,
            "skip_task_run",
            run_id,
            serde_json::json!({ "release_reason": reason, "dispatch_attempts": dispatch_attempts + 1 }),
        )
        .await?;
        tx.commit().await?;
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Skipped,
            lease_token: None,
        }));
    }

    // ⑧ 状态推进（R1：经统一守卫 validate_transition，与 Run 更新同事务）。
    apply_transition(&mut tx, task_id, TaskStatus::Ready, TaskStatus::Claimed, &ctx).await?;

    // ⑨ 租约签发（D 裁决）：明文只返回给 Model A；库内只存 sha256 小写 hex。
    //    Model B 不签发 → 写入 NULL（覆盖过期旧 Run 可能残留的 hash，验收 C17）。
    let (lease_token, lease_owner) = match opts.lease_owner {
        LeaseOwner::Worker(worker) => (None, worker.to_string()),
        LeaseOwner::Mcp { client_name } => {
            let token = crate::run_context::generate_lease_token()?;
            let prefix = token[..8].to_string();
            (Some(token), format!("mcp:{client_name}:{prefix}"))
        }
    };
    let lease_token_hash = lease_token
        .as_deref()
        .map(crate::run_context::hash_lease_token);

    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'claimed', stage = 'dispatch', claimed_at = now(), \
                lease_owner = $2, heartbeat_at = now(), \
                lease_expires_at = now() + make_interval(secs => $3::float8), \
                lease_token_hash = $4, client_name = $5, client_version = $6, \
                timeout_seconds = $7 \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(&lease_owner)
    .bind(cfg.lease_secs)
    .bind(&lease_token_hash)
    .bind(opts.client_name)
    .bind(opts.client_version)
    .bind(cfg.timeout_secs)
    .execute(&mut *tx)
    .await?;

    // R-F12：`git_branch` 是平台生成的普通字段（不属状态机），幂等回填即可。
    let git_branch = backfill_git_branch(&mut tx, task_id).await?;

    let baselines = snapshot_repositories(&mut tx, run_id, project_id, task_id).await?;

    // D2.1：完整策略快照（§15.5）。`policy_hash`/`policy_version` 为内容寻址，
    // `execution_enabled` 由策略门与审批证据推导，不再是字面量。
    let policy_snapshot = serde_json::json!({
        "policy_schema": POLICY_SCHEMA,
        "policy_version": assessment.policy_version,
        "policy_hash": assessment.policy_hash,
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "approval_required": assessment.approval_required,
        "approval_status": assessment.approval_status,
        "execution_enabled": assessment.execution_enabled,
        "repository_baselines": baselines.clone(),
        "snapshot_scope": POLICY_SCHEMA,
        "effective_policy": assessment.effective_policy.to_snapshot_json(),
    });
    sqlx::query("UPDATE delivery.task_run SET policy_snapshot = $2 WHERE id = $1")
        .bind(run_id)
        .bind(&policy_snapshot)
        .execute(&mut *tx)
        .await?;

    // Probe 模式：立即释放回 queued（不启动执行器）。
    if opts.mode == DispatchMode::Probe {
        requeue_for_redispatch(&mut tx, run_id).await?;
        sqlx::query("UPDATE delivery.task_run SET release_reason = 'probe_release' WHERE id = $1")
            .bind(run_id)
            .execute(&mut *tx)
            .await?;
    }

    audit_run(
        &mut tx,
        project_id,
        "claim_task_run",
        run_id,
        serde_json::json!({
            "task_id": task_id,
            "lease_owner": lease_owner,
            "lease_secs": cfg.lease_secs,
            "repositories": baselines,
            "git_branch": git_branch,
            "mode": format!("{:?}", opts.mode),
            "model": if opts.issue_token { "mcp" } else { "worker" },
            "client_name": opts.client_name,
            "client_version": opts.client_version,
        }),
    )
    .await?;

    tx.commit().await?;
    Ok(Some(ClaimOutcome {
        run_id,
        task_id,
        kind: ClaimKind::Claimed,
        lease_token,
    }))
}

/// 平台生成的任务分支名（C1）：格式 `task/<task_id 前 8 位>`，幂等回填。
/// 已存在（含人工指定）时保持原值，执行端不得自定分支名。
async fn backfill_git_branch(
    tx: &mut Transaction<'_, Postgres>,
    task_id: Uuid,
) -> Result<String, sqlx::Error> {
    let short = &task_id.to_string()[..8];
    let branch = format!("task/{short}");
    sqlx::query("UPDATE delivery.task SET git_branch = $2 WHERE id = $1 AND git_branch IS NULL")
        .bind(task_id)
        .bind(&branch)
        .execute(&mut **tx)
        .await?;
    sqlx::query_scalar("SELECT git_branch FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(&mut **tx)
        .await
}

/// R1：Task 状态变更经**统一受守卫服务**（`pm-domain::validate_transition` +
/// 行锁 + 审计）。`run_dispatch` 与既有 `update_task_status` 都经由同一条通道，
/// 因此不存在「第二条绕过状态机的写入路径」。`_from` 仅作调用点自述——实际前态
/// 由服务在行锁内从数据库读取并校验。
pub async fn apply_transition(
    tx: &mut Transaction<'static, Postgres>,
    task_id: Uuid,
    _from: TaskStatus,
    to: TaskStatus,
    ctx: &ExecutionContext,
) -> Result<(), RunDispatchError> {
    apply_task_transition(tx, task_id, None, to, &TransitionFacts::for_dispatch(), ctx)
        .await
        .map_err(|e| RunDispatchError::Transition(e.to_string()))?;
    Ok(())
}

/// 读取单一作用域层的策略声明；缺行返回 `None`（该层不表态）。
async fn load_scope_layer(
    tx: &mut Transaction<'_, Postgres>,
    scope: &str,
    scope_key: &str,
) -> Result<Option<PolicyLayer>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT auto_accept, auto_merge, auto_publish_knowledge, \
                require_human_approval, additional_checks \
           FROM delivery.policy_scope \
          WHERE scope = $1 AND scope_key = $2",
    )
    .bind(scope)
    .bind(scope_key)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| {
        Ok(PolicyLayer {
            auto_accept: row.get("auto_accept"),
            auto_merge: row.get("auto_merge"),
            auto_publish_knowledge: row.get("auto_publish_knowledge"),
            require_human_approval: row.get("require_human_approval"),
            additional_checks: decode_checks(row.get("additional_checks"))?,
        })
    })
    .transpose()
}

/// 任务级覆盖；缺行返回 `None`，`NULL` 列由 `PolicyLayer` 逐字段表达。
async fn load_task_layer(
    tx: &mut Transaction<'_, Postgres>,
    task_id: Uuid,
) -> Result<Option<PolicyLayer>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT auto_accept, auto_merge, auto_publish_knowledge, \
                require_human_approval, additional_checks \
           FROM delivery.task_policy_override \
          WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| {
        Ok(PolicyLayer {
            auto_accept: row.get("auto_accept"),
            auto_merge: row.get("auto_merge"),
            auto_publish_knowledge: row.get("auto_publish_knowledge"),
            require_human_approval: row.get("require_human_approval"),
            additional_checks: decode_checks(row.get("additional_checks"))?,
        })
    })
    .transpose()
}

/// 四层策略读取（§15.4）：organization → project → task_type → task。
async fn load_policy_layers(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    task_type: Option<&str>,
) -> Result<PolicyLayers, sqlx::Error> {
    let organization = load_scope_layer(tx, "organization", "*").await?;
    let project = load_scope_layer(tx, "project", &project_id.to_string()).await?;
    let task_type_layer = match task_type {
        Some(kind) => load_scope_layer(tx, "task_type", kind).await?,
        None => None,
    };
    let task = load_task_layer(tx, task_id).await?;
    Ok(PolicyLayers {
        organization,
        project,
        task_type: task_type_layer,
        task,
    })
}

/// `additional_checks` 必须是数组；脏数据上抛为策略读取失败，**不得静默忽略**。
fn decode_checks(value: serde_json::Value) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    serde_json::from_value(value).map_err(|error| sqlx::Error::Decode(Box::new(error)))
}

/// 有效审批评估（R5）：`required` 已由四层合成给出，因此任务级 `false`
/// 无法放宽高层 `true`。需要审批时必须存在 `approved` 证据；未决（pending）
/// 或缺证据（unknown）都**不得**默认放行。
async fn assess_approval(
    tx: &mut Transaction<'_, Postgres>,
    task_id: Uuid,
    required: bool,
) -> Result<(&'static str, bool), sqlx::Error> {
    if !required {
        return Ok(("approved", true));
    }
    let approved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.change_proposal cp \
           JOIN governance.approval_request ar ON ar.proposal_id = cp.id \
          WHERE cp.target_id = $1 AND ar.status = 'approved'",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if approved > 0 {
        return Ok(("approved", true));
    }
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.change_proposal cp \
           JOIN governance.approval_request ar ON ar.proposal_id = cp.id \
          WHERE cp.target_id = $1 AND ar.status = 'pending'",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if pending > 0 {
        return Ok(("pending", false));
    }
    Ok(("unknown", false))
}
        /// 可领取性判定的结果：`deny` 非空表示拒绝；策略字段供 `policy_snapshot` 记录。
pub struct ClaimAssessment {
    pub deny: Option<&'static str>,
    pub approval_required: bool,
    pub approval_status: &'static str,
    /// 策略门 + 审批证据的最终可执行判定；**不是**字面量，也不等同于 `auto_accept`
    /// （后者只决定验收是否自动化，不决定本次能否启动）。
    pub execution_enabled: bool,
    pub policy_version: String,
    pub policy_hash: String,
    pub effective_policy: EffectivePolicy,
}

/// 可领取性判定（§5.6 清单，全部通过才领取）。
async fn claimability_assessment(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    retry_exhausted: bool,
    task_type: Option<&str>,
) -> Result<ClaimAssessment, sqlx::Error> {
    let layers = load_policy_layers(tx, project_id, task_id, task_type).await?;
    let effective = EffectivePolicy::compose(&layers);
    let (approval_status, execution_enabled) =
        assess_approval(tx, task_id, effective.require_human_approval).await?;
    let policy_hash = effective.content_hash();
    let policy_version = policy_hash[..12].to_string();

    let mut assessment = ClaimAssessment {
        deny: None,
        approval_required: effective.require_human_approval,
        approval_status,
        execution_enabled,
        policy_version,
        policy_hash,
        effective_policy: effective,
    };
    if retry_exhausted {
        assessment.deny = Some("retry_exhausted");
        return Ok(assessment);
    }
    if !assessment.execution_enabled {
        assessment.deny = Some("policy_requires_approval");
        return Ok(assessment);
    }
    // 无活跃等待
    let waiting: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_waiting WHERE task_id = $1 AND ended_at IS NULL LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?;
    if waiting.is_some() {
        assessment.deny = Some("task_waiting");
        return Ok(assessment);
    }
    // 判定 4：已指派且执行端为 dev_agent。
    let assignee_is_executor: Option<bool> = sqlx::query_scalar(
        "SELECT (p.kind = 'dev_agent') FROM delivery.task t \
           JOIN core.principal p ON p.id = t.assignee_id \
          WHERE t.id = $1",
    )
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?;
    if assignee_is_executor != Some(true) {
        assessment.deny = Some("no_executor_assignee");
        return Ok(assessment);
    }
    // 至少一个可写仓库
    let writable: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_repository WHERE task_id = $1 AND write_access",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if writable == 0 {
        assessment.deny = Some("no_repository_scope");
        return Ok(assessment);
    }
    // 全部声明仓库已达 verified
    let unverified: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_repository tr \
           JOIN integration.project_repository pr \
             ON pr.id = tr.repository_id AND pr.project_id = tr.project_id \
          WHERE tr.task_id = $1 AND pr.status <> 'verified'",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if unverified > 0 {
        assessment.deny = Some("repository_not_verified");
        return Ok(assessment);
    }
    // 基线可解析
    let unbounded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_repository tr \
           JOIN integration.project_repository pr \
             ON pr.id = tr.repository_id AND pr.project_id = tr.project_id \
          WHERE tr.task_id = $1 AND coalesce(tr.base_commit, pr.head_commit_sha) IS NULL",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if unbounded > 0 {
        assessment.deny = Some("baseline_unavailable");
        return Ok(assessment);
    }
    Ok(assessment)
}

/// 写入本 Run 的多仓基线快照，返回给审计的摘要。
async fn snapshot_repositories(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT tr.repository_id, tr.write_access, \
                tr.base_ref, coalesce(tr.base_commit, pr.head_commit_sha) AS base_commit, \
                pr.name AS repo_name, tr.path_scope \
           FROM delivery.task_repository tr \
           JOIN integration.project_repository pr \
             ON pr.id = tr.repository_id AND pr.project_id = tr.project_id \
          WHERE tr.task_id = $1 \
          ORDER BY tr.repository_id",
    )
    .bind(task_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut summary = Vec::with_capacity(rows.len());
    for row in rows {
        let repository_id: Uuid = row.get("repository_id");
        let write_access: bool = row.get("write_access");
        let base_ref: String = row.get("base_ref");
        let base_commit: String = row.get("base_commit");
        let repo_name: String = row.get("repo_name");
        let path_scope: Option<Vec<String>> = row.get("path_scope");
        let workspace_path = workspace_path_for(repository_id, &repo_name);
        sqlx::query(
            "INSERT INTO delivery.task_run_repository \
             (run_id, project_id, repository_id, base_ref, base_commit, write_access, workspace_path, path_scope) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (run_id, repository_id) DO NOTHING",
        )
        .bind(run_id)
        .bind(project_id)
        .bind(repository_id)
        .bind(&base_ref)
        .bind(&base_commit)
        .bind(write_access)
        .bind(&workspace_path)
        .bind(&path_scope)
        .execute(&mut **tx)
        .await?;
        summary.push(serde_json::json!({
            "repository_id": repository_id,
            "base_commit": base_commit,
            "workspace_path": workspace_path,
        }));
    }
    Ok(summary)
}

/// 隔离已启动的执行器实例并记录终止结果（B3-2 的前置）。
async fn isolate_started_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    cfg: &RunDispatchConfig,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'timed_out', outcome = 'timed_out', finished_at = now(), \
                terminated_at = now(), termination_reason = 'lease_expired', \
                infrastructure_retry_count = infrastructure_retry_count + 1, \
                lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL \
          WHERE id = $1",
    )
    .bind(run_id)
    .execute(&mut **tx)
    .await?;
    // 失败记录不可变（append-only）；基础设施类失败不计入任务执行连续失败（B3-4）。
    sqlx::query(
        "INSERT INTO delivery.task_run_failure \
         (run_id, project_id, category, retryable, reason_code, timeout_seconds) \
         SELECT $1, project_id, 'infrastructure', true, 'lease_lost', $2 \
           FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .bind(cfg.lease_secs)
    .execute(&mut **tx)
    .await
    .ok();
    Ok(())
}

/// 退避回队（资格类不满足）：同一行回 `queued`，`dispatch_attempts+1`，指数退避。
async fn requeue_with_backoff(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    _dispatch_attempts: i32,
    cfg: &RunDispatchConfig,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL, \
                dispatch_attempts = dispatch_attempts + 1, \
                next_dispatch_at = now() + make_interval(secs => \
                    LEAST($3::float8, power(2, dispatch_attempts))::float8), \
                release_reason = $2 \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(reason)
    .bind(cfg.backoff_cap_secs)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

/// 同事务内把 Run 重置回可领取的 `queued`（不递增 dispatch_attempts）。
async fn requeue_for_redispatch(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL, \
                next_dispatch_at = now() \
          WHERE id = $1",
    )
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

/// Run 落 `cancelled`（任务终态/不存在），并留审计。
async fn cancel_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'cancelled', outcome = 'cancelled', finished_at = now(), \
                release_reason = $2, lease_owner = NULL, lease_expires_at = NULL, lease_token_hash = NULL \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(reason)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

/// 平台专用审计兜底 principal（`0015` 幂等种子）。
pub const DISPATCH_SERVICE_PRINCIPAL: Uuid = Uuid::from_u128(1);

/// 续租（§5.9，D3 执行器实例内部通道）：仅租约持有者可续。
///
/// D2 的**对外**续租通道是 [`crate::run_context::renew_lease_by_token`]：本函数
/// 以 `lease_owner` 为凭证且不写审计，不能作为 MCP 入口（见 D2 设计 §4.3）。
/// D2 起补上租约过期与 `running` 态检查——过期租约不得被续活。
pub async fn renew_lease(
    pool: &PgPool,
    run_id: Uuid,
    lease_owner: &str,
    lease_secs: i64,
) -> Result<bool, RunDispatchError> {
    let updated = sqlx::query(
        "UPDATE delivery.task_run \
            SET lease_expires_at = now() + make_interval(secs => $3::float8), heartbeat_at = now() \
          WHERE id = $1 AND lease_owner = $2 AND status IN ('claimed', 'running') \
            AND lease_expires_at > now()",
    )
    .bind(run_id)
    .bind(lease_owner)
    .bind(lease_secs)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// fencing（T25）：仅**当前 Run 的当前执行会话**可回传结果。被回收 / 被替代的旧
/// 执行实例（过期 session 或非活跃 Run）一律拒绝，防止迟到结果污染新 Run。
pub async fn accept_executor_result(
    pool: &PgPool,
    run_id: Uuid,
    executor_session: &str,
) -> Result<bool, RunDispatchError> {
    let current: Option<bool> = sqlx::query_scalar(
        "SELECT (status IN ('claimed', 'running') AND executor_session = $2) \
           FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .bind(executor_session)
    .fetch_optional(pool)
    .await?;
    Ok(current == Some(true))
}

/// 写一条 `governance.audit_log`（与业务变更同事务）。
///
/// actor 回退链（G6）：项目 owner human → 任意 human 成员 → 平台 service
/// principal（`ai-pm-dispatch`）。第三级保证「项目无 human 成员」时领取不会
/// 因 `RowNotFound` 失败，同时 `after_data` 强制携带 `client_name` / `lease_owner`
/// 保留真实触发者归因（评审 R-F7）。
pub(crate) async fn audit_run(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    action: &str,
    run_id: Uuid,
    after: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let actor: Option<Uuid> = sqlx::query_scalar(
        "SELECT p.id FROM core.project_member m \
           JOIN core.principal p ON p.id = m.principal_id \
          WHERE m.project_id = $1 AND m.role = 'owner' AND p.kind = 'human' AND p.is_active \
          LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;
    let actor = match actor {
        Some(id) => id,
        None => sqlx::query_scalar(
            "SELECT p.id FROM core.project_member m \
               JOIN core.principal p ON p.id = m.principal_id \
              WHERE m.project_id = $1 AND p.kind = 'human' AND p.is_active LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(DISPATCH_SERVICE_PRINCIPAL),
    };

    let mut after = after;
    let run_meta: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT lease_owner, client_name FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let (Some((lease_owner, client_name)), Some(map)) = (run_meta, after.as_object_mut()) {
        map.entry("lease_owner").or_insert(serde_json::json!(lease_owner));
        map.entry("client_name").or_insert(serde_json::json!(client_name));
    }

    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, action, target_type, target_id, after_data, source_kind) \
         VALUES ($1, $2, $3, 'task_run', $4, $5, 'run_dispatch')",
    )
    .bind(project_id)
    .bind(actor)
    .bind(action)
    .bind(run_id)
    .bind(after)
    .execute(&mut **tx)
    .await?;
    Ok(())
}