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
        }
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
pub async fn insert_task_run(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<CreatedRun, sqlx::Error> {
    let attempt_no: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(attempt_no), 0) + 1 FROM delivery.task_run WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;

    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, stage, next_dispatch_at) \
         VALUES ($1, $2, $3, 'queued', 'dispatch', now()) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimOutcome {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub kind: ClaimKind,
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
    let mut tx = pool.begin().await?;

    // ① 选候选 Run（不阻塞其他 Worker）。顺序：先锁 run。
    let candidate = sqlx::query(
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
    .await?;
    let Some(candidate) = candidate else {
        tx.commit().await?;
        return Ok(None);
    };
    let run_id: Uuid = candidate.get("id");
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
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Cancelled,
        }));
    };
    let task_state: String = task.get("status");
    let retry_exhausted: bool = task.get("retry_exhausted");
    let task_type: Option<String> = task.get("task_type");

    // ③ 任务已终态 / 被取代 → Run 是死记录，落 cancelled，不动任务。
    let terminal_or_gone = TaskStatus::from_str(&task_state)
        .map(|s| s.is_terminal())
        .unwrap_or(true);
    if terminal_or_gone {
        cancel_run(&mut tx, run_id, "task_terminal").await?;
        tx.commit().await?;
        return Ok(Some(ClaimOutcome {
            run_id,
            task_id,
            kind: ClaimKind::Cancelled,
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
            tx.commit().await?;
            return Ok(Some(ClaimOutcome {
                run_id,
                task_id,
                kind: ClaimKind::Claimed,
            }));
        }
        // B3-2：执行器已启动且失联 → 隔离旧实例、记录终止结果，再新建 Run。
        isolate_started_run(&mut tx, run_id, cfg).await?;
        let superseded_run_id = run_id;
        // 任务先退回 ready，统一走「正常领取」路径重建。
        if task_state == "claimed" || task_state == "running" {
            let from = TaskStatus::from_str(&task_state).unwrap_or(TaskStatus::Claimed);
            apply_transition(&mut tx, task_id, from, TaskStatus::Ready, &ctx).await?;
        }
        let created = insert_task_run(&mut tx, project_id, task_id).await?;
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
        tx.commit().await?;
        return Ok(Some(ClaimOutcome {
            run_id: created.run_id,
            task_id,
            kind: ClaimKind::Reclaimed,
        }));
    }

    // ⑤ 正常领取：任务必须严格为 ready。R4：调度关闭时不领取新 Run
    // （回收分支已在上方执行，不受开关影响）。
    if !mode.claims() {
        tx.commit().await?;
        return Ok(None);
    }
    if task_state != "ready" {
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
        }));
    }

    // ⑥ 可领取性判定（全部通过才领取）。
    let assessment =
        claimability_assessment(&mut tx, project_id, task_id, retry_exhausted, task_type.as_deref())
            .await?;
    if let Some(reason) = assessment.deny {
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
        }));
    }

    // ⑦ 状态推进（R1：经统一守卫 validate_transition，与 Run 更新同事务）。
    apply_transition(&mut tx, task_id, TaskStatus::Ready, TaskStatus::Claimed, &ctx).await?;

    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'claimed', stage = 'dispatch', claimed_at = now(), \
                lease_owner = $2, heartbeat_at = now(), \
                lease_expires_at = now() + make_interval(secs => $3::float8) \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(worker_id)
    .bind(cfg.lease_secs)
    .execute(&mut *tx)
    .await?;

    let baselines = snapshot_repositories(&mut tx, run_id, project_id, task_id).await?;

    // S3：D1 最小 policy_snapshot（兼容 0010 预置列；完整策略合成留 D2）。
    let policy_snapshot = serde_json::json!({
        "policy_version": serde_json::Value::Null,
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "approval_required": assessment.approval_required,
        "approval_status": assessment.approval_status,
        "execution_enabled": true,
        "repository_baselines": baselines.clone(),
        "snapshot_scope": "d1-minimal",
    });
    sqlx::query("UPDATE delivery.task_run SET policy_snapshot = $2 WHERE id = $1")
        .bind(run_id)
        .bind(&policy_snapshot)
        .execute(&mut *tx)
        .await?;

    // Probe 模式：立即释放回 queued（不启动执行器）。
    if mode == DispatchMode::Probe {
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
            "lease_owner": worker_id,
            "lease_secs": cfg.lease_secs,
            "repositories": baselines,
            "mode": format!("{mode:?}"),
        }),
    )
    .await?;

    tx.commit().await?;
    Ok(Some(ClaimOutcome {
        run_id,
        task_id,
        kind: ClaimKind::Claimed,
    }))
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

/// 有效审批评估（R5）：**多级合成** organization → project → task_type → task。
/// 任一层级要求人工审批即视为必须审批；任务级 `false` **不得**放宽高层的 `true`。
/// 需要审批时必须存在 `approved` 证据；未决（pending）或缺证据（unknown）都**不得
/// 默认放行**。各层级读取失败由 `Err` 上抛，调用方拒绝领取。
async fn assess_approval(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    task_type: Option<&str>,
) -> Result<(bool, &'static str, bool), sqlx::Error> {
    let organization: Option<bool> = sqlx::query_scalar(
        "SELECT require_human_approval FROM delivery.policy_scope \
          WHERE scope = 'organization' AND scope_key = '*'",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let project: Option<bool> = sqlx::query_scalar(
        "SELECT require_human_approval FROM delivery.policy_scope \
          WHERE scope = 'project' AND scope_key = $1",
    )
    .bind(project_id.to_string())
    .fetch_optional(&mut **tx)
    .await?;
    let task_type_level: Option<bool> = match task_type {
        Some(kind) => {
            sqlx::query_scalar(
                "SELECT require_human_approval FROM delivery.policy_scope \
                  WHERE scope = 'task_type' AND scope_key = $1",
            )
            .bind(kind)
            .fetch_optional(&mut **tx)
            .await?
        }
        None => None,
    };
    // 任务级（既有 override；NULL/缺行都视为该层不表态）。
    let task_level: Option<bool> = sqlx::query_scalar(
        "SELECT coalesce(require_human_approval, false) FROM delivery.task_policy_override \
          WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?;

    // 「更严者胜」：任一层级为 true 即要求审批（低层无法放宽高层）。
    let required = [organization, project, task_type_level, task_level]
        .into_iter()
        .flatten()
        .any(|value| value);
    if !required {
        return Ok((false, "approved", true));
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
        return Ok((true, "approved", true));
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
        return Ok((true, "pending", false));
    }
    Ok((true, "unknown", false))
}

/// 可领取性判定的结果：`deny` 非空表示拒绝；审批字段供 policy_snapshot 记录。
pub struct ClaimAssessment {
    pub deny: Option<&'static str>,
    pub approval_required: bool,
    pub approval_status: &'static str,
}

/// 可领取性判定（§5.6 清单，全部通过才领取）。
async fn claimability_assessment(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    retry_exhausted: bool,
    task_type: Option<&str>,
) -> Result<ClaimAssessment, sqlx::Error> {
    let mut assessment = ClaimAssessment {
        deny: None,
        approval_required: false,
        approval_status: "approved",
    };
    if retry_exhausted {
        assessment.deny = Some("retry_exhausted");
        return Ok(assessment);
    }
    let (required, status, enabled) = assess_approval(tx, project_id, task_id, task_type).await?;
    assessment.approval_required = required;
    assessment.approval_status = status;
    if !enabled {
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
                lease_owner = NULL, lease_expires_at = NULL \
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
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL, \
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
            SET status = 'queued', lease_owner = NULL, lease_expires_at = NULL, \
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
                release_reason = $2, lease_owner = NULL, lease_expires_at = NULL \
          WHERE id = $1",
    )
    .bind(run_id)
    .bind(reason)
    .execute(&mut **tx)
    .await
    .map(|_| ())
}

/// 续租（§5.9）：仅租约持有者可续。
pub async fn renew_lease(
    pool: &PgPool,
    run_id: Uuid,
    lease_owner: &str,
    lease_secs: i64,
) -> Result<bool, RunDispatchError> {
    let updated = sqlx::query(
        "UPDATE delivery.task_run \
            SET lease_expires_at = now() + make_interval(secs => $3::float8), heartbeat_at = now() \
          WHERE id = $1 AND lease_owner = $2 AND status = 'claimed'",
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

/// 写一条 `governance.audit_log`（与业务变更同事务）。actor 取项目 owner（三级回退）。
async fn audit_run(
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

        .ok_or(sqlx::Error::RowNotFound)?,
    };
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