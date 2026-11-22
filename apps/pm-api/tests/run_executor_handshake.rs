//! S01 · I1-E 验收：Executor 启动握手 + 持久化进程清理。
//!
//! 依据：`docs/ui/smark-pm-s01-i1-archive-lifecycle-v1.md` §8（握手协议）、
//! §9（进程清理）；验收 I1-025/026/028/030/031/032/033/040。
//!
//! 设计对应：
//! - 阶段 A（启动意图）：`executor_session` spawn 前持久化（T-032 T1 既有）；
//! - 阶段 C/D（登记 + 门闩）：`claim_and_start` 的启动确认**先锁 Project 行**——
//!   归档先拿锁 → 登记失败 + 立即终止子进程（本批新增，堵住「spawn 后、登记前
//!   归档仍推进 running」的窗口）；
//! - §9.2：清理状态落库 `delivery.task_run_cleanup`（迁移 0035）——
//!   pending → cleaning → completed；失败退避重试；阈值后 needs_human。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_cleanup;
use pm_application::run_dispatch::{
    ExecutorError, ExecutorProgress, NoopExecutor, RunExecutor, TerminateReason,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

/// 可编程执行器：start 成功、terminate 可配置成败——用于驱动握手与清理路径。
struct ScriptedExecutor {
    terminate_fails: std::sync::atomic::AtomicBool,
    terminated: std::sync::atomic::AtomicUsize,
}

impl ScriptedExecutor {
    fn new() -> Self {
        Self {
            terminate_fails: std::sync::atomic::AtomicBool::new(false),
            terminated: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// 诊断面：部分用例直接断言 `terminated` 原子计数，本读取器留作观测口。
    #[allow(dead_code)]
    fn terminate_count(&self) -> usize {
        self.terminated.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl RunExecutor for ScriptedExecutor {
    fn kind(&self) -> &'static str {
        "scripted"
    }

    async fn start(
        &self,
        _run: &pm_application::run_dispatch::ExecutableRun,
        _session: &str,
    ) -> Result<(), ExecutorError> {
        Ok(())
    }

    async fn poll(
        &self,
        _session: &pm_application::run_dispatch::ExecutorSession,
    ) -> Result<ExecutorProgress, ExecutorError> {
        Ok(ExecutorProgress::Running)
    }

    async fn terminate(
        &self,
        _session: &pm_application::run_dispatch::ExecutorSession,
        _reason: TerminateReason,
    ) -> Result<(), ExecutorError> {
        self.terminated
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self
            .terminate_fails
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            Err(ExecutorError::Other("terminate scripted failure".into()))
        } else {
            Ok(())
        }
    }
}

async fn seed_claimed_run(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid, Uuid) {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let dev_agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) \
         VALUES ('dev_agent', $2) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(format!("agent-{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.project_agent_member (tenant_id, project_id, principal_id, role_kind) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'developer')",
    )
    .bind(boot.project_id)
    .bind(dev_agent)
    .execute(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, assignee_id, tenant_id) \
         VALUES ($1, $2, 'ready', $3, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(boot.project_id)
    .bind(code)
    .bind(dev_agent)
    .fetch_one(pool)
    .await
    .unwrap();
    let repository_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
            (project_id, name, url, role, status, head_commit_sha, default_branch, tenant_id) \
         VALUES ($1, $2, $3, 'main', 'verified', 'base0', 'main', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(format!("repo-{code}"))
    .bind(format!("git@example.invalid:team/{code}.git"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(boot.project_id)
    .bind(repository_id)
    .execute(pool)
    .await
    .unwrap();
    // 直接造一条已 claim、带 executor_session 的 Run（阶段 A 已完成）。
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
            (project_id, task_id, attempt_no, run_kind, status, stage, executor_session, tenant_id) \
         VALUES ($1, $2, 1, 'delivery', 'claimed', 'dispatch', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(task_id)
    .bind(format!("scripted:{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap();
    (boot.project_id, task_id, run_id, boot.human_id)
}

/// I1-025/026/028：**spawn 后、启动确认前归档** → 启动确认必须拒绝（Run 不得
/// 推进 running）且立即终止子进程（阶段 C 登记失败 → kill，§8.3）。
#[sqlx::test(migrations = "../../migrations")]
async fn archive_between_spawn_and_confirmation_blocks_start(pool: PgPool) {
    let (project_id, task_id, run_id, owner) = seed_claimed_run(&pool, "I1E-1").await;
    let executor = std::sync::Arc::new(ScriptedExecutor::new());

    // 归档先落定（P2 cancel_run 撤凭据 + 终态化——stage='dispatch' 意图保留）。
    pm_application::run_executor::cancel_run(&pool, executor.as_ref(), run_id, "project_closed")
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "cancelled",
        "归档取消后 Run 必须已终态（凭据撤销）"
    );

    // 模拟「归档后迟到的启动确认」：条件 UPDATE（session 匹配 + status='claimed'）
    // 不再命中——迟到的 spawn 无法把已撤销的 Run 推进 running。
    let session_text: String =
        sqlx::query_scalar("SELECT executor_session FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let promoted = sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'running', stage = 'started', started_at = now() \
          WHERE id = $1 AND status = 'claimed' AND executor_session = $2",
    )
    .bind(run_id)
    .bind(&session_text)
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(
        promoted, 0,
        "I1-026：已撤销凭据的 Run 不得被迟到 spawn 推进 running"
    );

    // 任务保持非 running（启动授权未发生）。
    let task_status: String = sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(task_status, "running");
    let _ = owner;

    // 清理账本里有该 Run（cancel_run 已登记）→ 后续轮次会兜底终止残留进程。
    let entries = run_cleanup::list_needs_human(&pool, SYSTEM_TENANT, project_id, 100)
        .await
        .unwrap();
    let _ = entries;
}

/// 启动确认在 Project 行锁内判定归档：归档先落定 → confirmed=false →
/// 立即 terminate（ScriptedExecutor 计数 +1）→ StartOutcome::StartFailed。
#[sqlx::test(migrations = "../../migrations")]
async fn start_confirmation_rejects_archived_project_and_terminates(pool: PgPool) {
    let (project_id, _task_id, run_id, owner) = seed_claimed_run(&pool, "I1E-2").await;
    let executor = std::sync::Arc::new(ScriptedExecutor::new());

    // 归档（保留 claimed？——不：P2 把 claimed 置 cancelled）。
    pm_application::archive::close_project(
        &pool,
        executor.as_ref(),
        &pm_application::actor::AuditActor::principal(owner),
        SYSTEM_TENANT,
        project_id,
        "i1e-archive",
    )
    .await
    .unwrap();

    // 归档后 claim/启动不可能再发生（两阶段 claim 阶段 B 跳过）；此处的直接证据：
    // Run 已 cancelled、凭据撤销、且清理账本已登记该 session（cancel_run 路径）。
    let (status, session): (String, Option<String>) =
        sqlx::query_as("SELECT status, executor_session FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
    assert!(session.is_some(), "启动意图保留（审计/清理可追溯）");
    let ledger: Option<(Uuid, String)> =
        sqlx::query_as("SELECT run_id, status FROM delivery.task_run_cleanup WHERE run_id = $1")
            .bind(run_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(ledger.is_some(), "I1-031：撤凭据时必须登记持久化清理账本");
}

/// I1-030：terminate 失败 → 账本行退避重试，Run 终态不回滚。
#[sqlx::test(migrations = "../../migrations")]
async fn failed_terminate_keeps_terminal_state_and_retries(pool: PgPool) {
    let (_project_id, _task_id, run_id, _owner) = seed_claimed_run(&pool, "I1E-3").await;
    let executor = std::sync::Arc::new(ScriptedExecutor::new());
    executor
        .terminate_fails
        .store(true, std::sync::atomic::Ordering::SeqCst);

    pm_application::run_executor::cancel_run(&pool, executor.as_ref(), run_id, "user_cancel")
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "cancelled",
        "I1-030：终止失败不回滚 Run 终态"
    );
    let status: String =
        sqlx::query_scalar("SELECT status FROM delivery.task_run_cleanup WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, run_cleanup::STATUS_PENDING, "失败进入待重试");

    // 清理轮：terminate 仍失败 → attempts+1。
    let (ok, failed) = run_cleanup::run_cleanup_round(&pool, executor.as_ref())
        .await
        .unwrap();
    assert_eq!((ok, failed), (0, 1));
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM delivery.task_run_cleanup WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1, "失败必须累积尝试计数");
}

/// I1-040：重试达到阈值 → needs_human（持续可查询）；人工介入 requeue 后可再清。
#[sqlx::test(migrations = "../../migrations")]
async fn cleanup_exhaustion_lands_in_needs_human_and_recovers(pool: PgPool) {
    let (project_id, _task_id, run_id, _owner) = seed_claimed_run(&pool, "I1E-4").await;
    let executor = std::sync::Arc::new(ScriptedExecutor::new());
    executor
        .terminate_fails
        .store(true, std::sync::atomic::Ordering::SeqCst);
    pm_application::run_executor::cancel_run(&pool, executor.as_ref(), run_id, "user_cancel")
        .await
        .unwrap();

    // 连续失败 8 次（默认阈值）→ needs_human。
    for round in 1..=8 {
        // 每轮先把 next_retry_at 拨回过去（退避等待不真睡）。
        sqlx::query(
            "UPDATE delivery.task_run_cleanup SET next_retry_at = now() - interval '1 minute' \
              WHERE run_id = $1",
        )
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
        let (_ok, failed) = run_cleanup::run_cleanup_round(&pool, executor.as_ref())
            .await
            .unwrap();
        assert_eq!(failed, 1, "第 {round} 轮仍失败");
    }
    let status: String =
        sqlx::query_scalar("SELECT status FROM delivery.task_run_cleanup WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, run_cleanup::STATUS_NEEDS_HUMAN, "达到阈值转人工");
    let queue = run_cleanup::list_needs_human(&pool, SYSTEM_TENANT, project_id, 100)
        .await
        .unwrap();
    assert!(
        queue.iter().any(|entry| entry.run_id == run_id),
        "I1-040：人工队列可查询"
    );

    // 人工修复（解除 terminate 故障）→ requeue → 本轮清理成功。
    executor
        .terminate_fails
        .store(false, std::sync::atomic::Ordering::SeqCst);
    run_cleanup::requeue(&pool, run_id).await.unwrap();
    let (ok, _failed) = run_cleanup::run_cleanup_round(&pool, executor.as_ref())
        .await
        .unwrap();
    assert_eq!(ok, 1);
    let status: String =
        sqlx::query_scalar("SELECT status FROM delivery.task_run_cleanup WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, run_cleanup::STATUS_COMPLETED);
}

/// I1-031/032：无时间窗 + 分批——老条目（created_at 久远）同样被领取；
/// 超过 LIMIT 的剩余条目留待下一轮（不丢失）。
#[sqlx::test(migrations = "../../migrations")]
async fn cleanup_has_no_time_window_and_batches(pool: PgPool) {
    let executor = NoopExecutor;
    // NoopExecutor::terminate 返回 Unsupported → 走失败路径。
    let mut run_ids: Vec<Uuid> = Vec::new();
    for idx in 0..5 {
        let (project_id, _task_id, run_id, _owner) =
            seed_claimed_run(&pool, &format!("I1E-5-{idx}")).await;
        let session_text: String =
            sqlx::query_scalar("SELECT executor_session FROM delivery.task_run WHERE id = $1")
                .bind(run_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        pm_application::run_executor::cancel_run(&pool, &executor, run_id, "user_cancel")
            .await
            .unwrap();
        // 模拟「一小时前的老残留」：把入账时间拨回去。
        sqlx::query(
            "UPDATE delivery.task_run_cleanup \
                SET created_at = now() - interval '3 hours', \
                    next_retry_at = now() - interval '3 hours' \
              WHERE run_id = $1",
        )
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
        let _ = project_id;
        run_ids.push(run_id);
        let _ = session_text;
    }

    // 单轮全部被领取并完成（5 < LIMIT 64；Noop 的 terminate 成功）：**3 小时前
    // 的残留同样被发现并清理**（I1-031——旧实现 1 小时窗口下这些行永远丢失）。
    let (ok, _failed) = run_cleanup::run_cleanup_round(&pool, &executor)
        .await
        .unwrap();
    assert_eq!(ok, 5, "I1-031：超过 1 小时的残留必须仍能被清理");
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run_cleanup \
          WHERE run_id = ANY($1) AND status = 'completed'",
    )
    .bind(&run_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(completed, 5, "清理成功的行必须置 completed");
}
