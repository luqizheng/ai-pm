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

/// T-032 §6.2：workspace_path 的执行器侧二次校验（防御层）。算法本身
/// （[`workspace_path_for`]）从字符层已安全，本函数防「DB 值被手工改坏后穿越」：
/// 规范化后断言无父目录 / 根分量、非绝对路径。返回规范化后的相对路径。
pub fn validate_workspace_path(raw: &str) -> Result<String, ExecutorError> {
    use std::path::{Component, Path};
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(ExecutorError::Other(format!(
            "workspace_path must be relative: {raw}"
        )));
    }
    let mut clean = Vec::new();
    for comp in path.components() {
        match comp {
            Component::Normal(part) => clean.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            other => {
                return Err(ExecutorError::Other(format!(
                    "workspace_path rejected (component {other:?}): {raw}"
                )));
            }
        }
    }
    Ok(clean.join("/"))
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
    /// T-032 T2：领取时的执行权世代（首次领取=1，重领递增）。执行端 MCP 双验证用。
    pub lease_generation: i32,
    /// T-032 ADR-0002 L1/L2：一次性租约明文（**Model A / Model B 均签发**），
    /// 经 `AI_PM_LEASE_TOKEN` 注入执行端环境；禁止落入任何日志与 pid 文件。
    pub lease_token: Option<String>,
    /// T-032 T3：绝对执行截止（首次 claim 固化；重领/续租不得重置或延长）。
    pub deadline_at: Option<chrono::DateTime<chrono::Utc>>,
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

/// T-032 §6.5：向 Run 目录追加一条结构化执行事件（`executor-events.jsonl`）。
/// 编排器与适配器共用；写失败不阻断主流程（事件是尽力而为的可观测性）。
pub fn append_executor_event(root: &str, run_id: Uuid, event: &serde_json::Value) {
    use std::io::Write;
    let dir = std::path::Path::new(root).join(run_id.to_string()).join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("executor-events.jsonl");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{event}");
    }
}


/// 执行端会话（D3 填充）。`session` 格式 `{kind}:{uuid-v4}`（T-030 Q-D3-6）；
/// `workspace_root` 是本 Run 的运行期目录根（`{AI_PM_WORKSPACE_ROOT}/{run_id}`），
/// 编排器与适配器据此定位 pid 文件与日志。
///
/// T-032 T1 语义：`executor_session` 是**启动意图**（spawn 前持久化），
/// 不等于进程已经启动——进程身份以 pid 文件 + 启动确认为准。
#[derive(Debug, Clone)]
pub struct ExecutorSession {
    pub session: String,
    pub run_id: Uuid,
    pub workspace_root: String,
}


/// T-032 §6.4：结构化退出结果（替代裸 `Option<i32>`）。
///
/// `Killed` 只会出现在**平台主动终止**路径（编排器构造），`poll` 收割自然退出的
/// 进程只会产出其余四类；`Lost` = 跨重启孤儿退出码不可归因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitOutcome {
    /// 进程正常退出且退出码为 0。
    Success,
    /// 进程退出且退出码非 0。
    NonZeroExit { code: i32 },
    /// 进程被信号终止（Unix；Windows 无此语义）。
    Signaled { signal: i32 },
    /// 平台主动终止（用户取消 / 超时 / 心跳丢失 / 进展停滞 / 租约回收）。
    Killed { reason: TerminateReason },
    /// 孤儿跨重启后退出码不可归因（pid 证据消失 / 进程不存在）。
    Lost,
}


/// 一次轮询得到的阶段进度。
///
/// T-032 T1 终裁：`Uncertain` = 跨重启孤儿且**三重证据**（pid 文件 / run 目录
/// 活跃痕迹 / Job 对象或进程组）无法确证进程已死——按「可能存活」处理，编排器
/// 必须告警并保留回收任务，**不得**静默判定回收。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorProgress {
    Pending,
    Running,
    /// 进程已退出并收割，携带结构化退出结果。
    Done { outcome: ExitOutcome },
    Uncertain,
}


/// 终止信号的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminateReason {
    Cancelled,
    TimedOut,
    LeaseReclaimed,
    /// T-032 T4：Agent 心跳失效（进程存活但心跳停滞超阈值）。
    HeartbeatLost,
    /// T-032 T4：长期无进展（`last_progress_at` 停滞超阈值）。
    ProgressStalled,
}


#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("executor does not support this operation")]
    Unsupported,
    #[error("executor failure: {0}")]
    Other(String),
}

/// 执行器端口（D1 只定义生命周期契约，D3 再实现真实执行器）。
///
/// T-032 T1：`start` 接收**编排器预生成的 session**——编排器在 spawn 前先把
/// `executor_session` 持久化（启动意图），随后才调用 `start` spawn 进程；
/// 适配器不得自行生成 session。返回值只表达「spawn 是否成功」。
#[async_trait::async_trait]
pub trait RunExecutor: Send + Sync {
    fn kind(&self) -> &'static str;
    async fn start(&self, run: &ExecutableRun, session: &str) -> Result<(), ExecutorError>;
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

    async fn start(&self, _run: &ExecutableRun, _session: &str) -> Result<(), ExecutorError> {
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
