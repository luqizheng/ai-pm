use super::*;

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
         (project_id, task_id, attempt_no, status, stage, next_dispatch_at, source_event_id, tenant_id) \
         VALUES ($1, $2, $3, 'queued', 'dispatch', now(), $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
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
    /// D2 契约 `run.timeout_seconds` 的库内值；`None` = 未配置（不超时判定）。
    pub timeout_seconds: Option<i32>,
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


/// 运行期工作区根（T-030 Q-D3-2）：`{root}/{run_id}/{workspace_path}`。
/// 编排器与适配器共用同一派生规则，进程重启后仍能定位既有目录。
pub fn workspace_root_from_env() -> String {
    std::env::var("AI_PM_WORKSPACE_ROOT").unwrap_or_else(|_| "./run-workspaces".to_string())
}


/// 执行端会话（D3 填充）。`session` 格式 `{kind}:{uuid-v4}`（T-030 Q-D3-6）；
/// `workspace_root` 是本 Run 的运行期目录根（`{AI_PM_WORKSPACE_ROOT}/{run_id}`），
/// 编排器与适配器据此定位 pid 文件与日志。
#[derive(Debug, Clone)]
pub struct ExecutorSession {
    pub session: String,
    pub run_id: Uuid,
    pub workspace_root: String,
}


/// 一次轮询得到的阶段进度。`Done` 携带进程退出码：`Some(0)` = 正常结束；
/// `Some(n≠0)` = 执行失败；`None` = 被信号/外部终止（退出码不可归因）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorProgress {
    Pending,
    Running,
    Done { exit_code: Option<i32> },
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


