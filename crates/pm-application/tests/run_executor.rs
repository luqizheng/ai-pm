//! T-031 编排层测试（`docs/slice-d/ai-pm-slice-d3-executor-adapter-v1.md` §5 第一层）：
//! FakeExecutor 驱动 claim_and_start / poll_active_runs / cancel_run /
//! reconcile_orphans 的编排语义，验收 C59–C66。真实进程管理见
//! `pm-infrastructure/tests/codex_executor.rs`。

use std::sync::Mutex;

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_dispatch::{
    claim_once, ClaimKind, DispatchMode, RunDispatchConfig, RunExecutor,
};
use pm_application::run_executor::{
    cancel_run, claim_and_start, poll_active_runs, reconcile_orphans, StartOutcome,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

/// 注入行为（C59–C66 的 fail-injection 第一层）。
#[derive(Debug, Clone)]
enum Behavior {
    /// start 成功，进程立即以该退出码结束。
    Exit(i32),
    /// start 成功，进程一直挂着（供超时/取消/失联路径）。
    Hang,
    /// start 直接失败（spawn 层错误）。
    StartFail(String),
}

struct FakeExecutor {
    behavior: Mutex<Behavior>,
    started: Mutex<Vec<String>>,
    terminated: Mutex<Vec<String>>,
}

impl FakeExecutor {
    fn new(behavior: Behavior) -> Self {
        Self {
            behavior: Mutex::new(behavior),
            started: Mutex::new(Vec::new()),
            terminated: Mutex::new(Vec::new()),
        }
    }

    fn started_count(&self) -> usize {
        self.started.lock().unwrap().len()
    }

    fn terminated_count(&self) -> usize {
        self.terminated.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl RunExecutor for FakeExecutor {
    fn kind(&self) -> &'static str {
        "fake"
    }

    async fn start(
        &self,
        _run: &pm_application::run_dispatch::ExecutableRun,
        session: &str,
    ) -> Result<(), pm_application::run_dispatch::ExecutorError> {
        let behavior = self.behavior.lock().unwrap().clone();
        match behavior {
            Behavior::StartFail(detail) => {
                Err(pm_application::run_dispatch::ExecutorError::Other(detail))
            }
            // T-032 T1：session 由编排器前置生成并持久化（启动意图），适配器
            // 只接收与记录——不再自行生成。
            Behavior::Exit(_) | Behavior::Hang => {
                self.started.lock().unwrap().push(session.to_string());
                Ok(())
            }
        }
    }

    async fn poll(
        &self,
        _session: &pm_application::run_dispatch::ExecutorSession,
    ) -> Result<
        pm_application::run_dispatch::ExecutorProgress,
        pm_application::run_dispatch::ExecutorError,
    > {
        use pm_application::run_dispatch::ExitOutcome;
        let behavior = self.behavior.lock().unwrap().clone();
        Ok(match behavior {
            Behavior::Exit(code) => pm_application::run_dispatch::ExecutorProgress::Done {
                outcome: if code == 0 {
                    ExitOutcome::Success
                } else {
                    ExitOutcome::NonZeroExit { code }
                },
            },
            Behavior::Hang => pm_application::run_dispatch::ExecutorProgress::Running,
            Behavior::StartFail(_) => pm_application::run_dispatch::ExecutorProgress::Done {
                outcome: ExitOutcome::Lost,
            },
        })
    }

    async fn terminate(
        &self,
        session: &pm_application::run_dispatch::ExecutorSession,
        _reason: pm_application::run_dispatch::TerminateReason,
    ) -> Result<(), pm_application::run_dispatch::ExecutorError> {
        self.terminated
            .lock()
            .unwrap()
            .push(session.session.clone());
        Ok(())
    }
}

// ------------------------------------------------------------------ fixtures

struct Fx {
    task_id: Uuid,
    run_id: Uuid,
}

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
        timeout_secs: 1800,
        deadline_default_secs: 3600,
        agent_heartbeat_stale_secs: 900,
        progress_stale_secs: 3600,
        exclude_projects: vec![],
    }
}

async fn seed(pool: &PgPool, code: &str) -> Fx {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', $1) RETURNING id",
    )
    .bind(format!("dev {code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, 't', 'ready', 1, $2, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(agent)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, $2, $3, 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(format!("repo-{code}"))
    .bind(format!("git@host:{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository \
         (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(outcome.project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, timeout_seconds, tenant_id) \
         VALUES ($1, $2, 1, 'queued', NULL, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Fx { task_id, run_id }
}

async fn run_status(pool: &PgPool, run_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task_run WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn set_behavior_ready_now(pool: &PgPool, run_id: Uuid) {
    sqlx::query(
        "UPDATE delivery.task_run SET next_dispatch_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(run_id)
    .execute(pool)
    .await
    .unwrap();
}

// ---------------------------------------------------------------- C59 / C62

#[sqlx::test(migrations = "../../migrations")]
async fn c59_full_loop_exit_zero_succeeds_run(pool: PgPool) {
    let fx = seed(&pool, "RE-C59").await;
    let executor = FakeExecutor::new(Behavior::Exit(0));
    let cfg = cfg();

    let outcome = claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();
    let StartOutcome::Started { run_id, session } = outcome else {
        panic!("expected Started, got {outcome:?}");
    };
    assert_eq!(run_id, fx.run_id);
    assert!(session.starts_with("fake:"));

    // 编排器推进：Run running + stage=started；任务经受守卫入口 claimed→running。
    assert_eq!(run_status(&pool, fx.run_id).await, "running");
    assert_eq!(task_status(&pool, fx.task_id).await, "running");
    let (kind, session_db): (String, String) = sqlx::query_as(
        "SELECT executor_kind, executor_session FROM delivery.task_run WHERE id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "fake");
    assert_eq!(session_db, session);

    let handled = poll_active_runs(&pool, &executor, "w1", &cfg)
        .await
        .unwrap();
    assert_eq!(handled, 1);
    assert_eq!(run_status(&pool, fx.run_id).await, "succeeded");
    // 任务级裁决归 D5：D3 收尾后任务停在 running。
    assert_eq!(task_status(&pool, fx.task_id).await, "running");
    let failures: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_run_failure WHERE run_id = $1")
            .bind(fx.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(failures, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c62_exit_nonzero_fails_run_without_task_verdict(pool: PgPool) {
    let fx = seed(&pool, "RE-C62").await;
    let executor = FakeExecutor::new(Behavior::Exit(3));
    let cfg = cfg();

    assert!(matches!(
        claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
            .await
            .unwrap(),
        StartOutcome::Started { .. }
    ));
    poll_active_runs(&pool, &executor, "w1", &cfg)
        .await
        .unwrap();

    assert_eq!(run_status(&pool, fx.run_id).await, "failed");
    assert_eq!(task_status(&pool, fx.task_id).await, "running");
    let (category, exit_code): (String, Option<i32>) = sqlx::query_as(
        "SELECT category, exit_code FROM delivery.task_run_failure WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((category.as_str(), exit_code), ("execution", Some(3)));
}

// ---------------------------------------------------------------- C60 / C61

#[sqlx::test(migrations = "../../migrations")]
async fn c60_timeout_terminates_and_marks_timed_out(pool: PgPool) {
    let fx = seed(&pool, "RE-C60").await;
    let executor = FakeExecutor::new(Behavior::Hang);
    let cfg = cfg();

    claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();
    // T-032 T3 终裁：deadline_at 在首次 claim 时已固化（= now + timeout_seconds）
    // ——事后改 timeout_seconds 不影响在跑 Run；测试造超时改为直接压缩 deadline。
    sqlx::query(
        "UPDATE delivery.task_run SET deadline_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    poll_active_runs(&pool, &executor, "w1", &cfg)
        .await
        .unwrap();

    assert_eq!(run_status(&pool, fx.run_id).await, "timed_out");
    assert_eq!(executor.terminated_count(), 1);
    let (reason, category): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT termination_reason, (SELECT category FROM delivery.task_run_failure WHERE run_id = r.id) \
           FROM delivery.task_run r WHERE r.id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reason.as_deref(), Some("timeout"));
    assert_eq!(category.as_deref(), Some("execution"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn c61_user_cancel_uses_the_same_pipeline(pool: PgPool) {
    let fx = seed(&pool, "RE-C61").await;
    let executor = FakeExecutor::new(Behavior::Hang);
    let cfg = cfg();

    claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();

    let cancelled = cancel_run(&pool, &executor, fx.run_id, "user_cancel")
        .await
        .unwrap();
    assert!(cancelled);
    assert_eq!(run_status(&pool, fx.run_id).await, "cancelled");
    assert_eq!(executor.terminated_count(), 1);
    let reason: Option<String> =
        sqlx::query_scalar("SELECT termination_reason FROM delivery.task_run WHERE id = $1")
            .bind(fx.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("user_cancel"));
}

// ---------------------------------------------------------------- C63 / C64

#[sqlx::test(migrations = "../../migrations")]
async fn c63_start_failure_requeues_with_infrastructure_counting(pool: PgPool) {
    let fx = seed(&pool, "RE-C63").await;
    let executor = FakeExecutor::new(Behavior::StartFail("binary missing".into()));
    let cfg = cfg();

    let outcome = claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();
    let StartOutcome::StartFailed { run_id, error } = outcome else {
        panic!("expected StartFailed, got {outcome:?}");
    };
    assert_eq!(run_id, fx.run_id);
    assert!(error.contains("binary missing"));

    assert_eq!(run_status(&pool, fx.run_id).await, "queued");
    let (reason, retries): (Option<String>, i32) = sqlx::query_as(
        "SELECT release_reason, infrastructure_retry_count FROM delivery.task_run WHERE id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reason.as_deref(), Some("executor_start_failed"));
    assert_eq!(retries, 1);
    let failures: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_run_failure WHERE run_id = $1")
            .bind(fx.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(failures, 1);
    // 启动失败不改任务状态：任务仍 ready，等下一次领取。
    assert_eq!(task_status(&pool, fx.task_id).await, "ready");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c64_infrastructure_retry_exhaustion_fails_the_run(pool: PgPool) {
    let fx = seed(&pool, "RE-C64").await;
    let executor = FakeExecutor::new(Behavior::StartFail("still missing".into()));
    let cfg = RunDispatchConfig {
        max_infrastructure_attempts: 2,
        ..cfg()
    };

    for _ in 0..2 {
        claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
            .await
            .unwrap();
        set_behavior_ready_now(&pool, fx.run_id).await;
    }

    assert_eq!(run_status(&pool, fx.run_id).await, "failed");
    // 失败行每 Run 至多一条（0010）：仍是首败的 executor_start_failed；
    // 耗尽证据在 release_reason 与审计里。
    let release_reason: Option<String> =
        sqlx::query_scalar("SELECT release_reason FROM delivery.task_run WHERE id = $1")
            .bind(fx.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(release_reason.as_deref(), Some("infrastructure_exhausted"));
    let (failures, exhausted_audits): (i64, i64) = sqlx::query_as(
        "SELECT \
             (SELECT count(*) FROM delivery.task_run_failure WHERE run_id = $1), \
             (SELECT count(*) FROM governance.audit_log \
               WHERE action = 'run_start_failed' AND target_id = $1 \
                 AND after_data->>'infrastructure_retry_count' = '2')",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failures, 1);
    assert_eq!(exhausted_audits, 1);
    // 第三轮没有可领取的候选（Run 已终态）。
    let outcome = claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();
    assert!(matches!(outcome, StartOutcome::Idle));
    assert_eq!(executor.started_count(), 0);
}

// ---------------------------------------------------------------- C65 / C66

#[sqlx::test(migrations = "../../migrations")]
async fn c65_running_run_with_unrenewable_lease_is_reclaimed_as_lost(pool: PgPool) {
    let fx = seed(&pool, "RE-C65").await;
    let executor = FakeExecutor::new(Behavior::Hang);
    let cfg = cfg();

    claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();
    assert_eq!(run_status(&pool, fx.run_id).await, "running");
    // 模拟长失联：租约过期且 worker 续不上（renew_lease 拒绝过期租约）。
    sqlx::query(
        "UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    let handled = poll_active_runs(&pool, &executor, "w1", &cfg)
        .await
        .unwrap();
    assert_eq!(handled, 1);
    assert_eq!(
        executor.terminated_count(),
        1,
        "lost run must be terminated"
    );

    // B3-2：旧 Run 落 timed_out(termination_reason='lease_expired'，
    // failure reason_code='lease_lost')，新 Run supersedes 旧 Run。
    assert_eq!(run_status(&pool, fx.run_id).await, "timed_out");
    let reason: Option<String> =
        sqlx::query_scalar("SELECT termination_reason FROM delivery.task_run WHERE id = $1")
            .bind(fx.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("lease_expired"));
    let superseded: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_run WHERE supersedes_run_id = $1")
            .bind(fx.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(superseded, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c66_concurrent_finalization_wins_over_poll(pool: PgPool) {
    let fx = seed(&pool, "RE-C66").await;
    let executor = FakeExecutor::new(Behavior::Exit(0));
    let cfg = cfg();

    claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();
    // 模拟并发收尾（如 submit_result 通道先落终态）。
    sqlx::query(
        "UPDATE delivery.task_run SET status = 'succeeded', outcome = 'succeeded', finished_at = now() \
          WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    // 并发收尾后 Run 已不在活跃集合：poll 查不到候选，绝不二次收尾。
    let handled = poll_active_runs(&pool, &executor, "w1", &cfg)
        .await
        .unwrap();
    assert_eq!(handled, 0, "finalized run leaves the active set");
    let finished_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE action = 'run_finished' AND target_id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        finished_audits, 0,
        "conditional UPDATE must not double-finalize"
    );
    assert_eq!(run_status(&pool, fx.run_id).await, "succeeded");
}

/// reconcile：孤儿 Run 退出码可归因（Some(0)）→ 直接收尾 succeeded。
#[sqlx::test(migrations = "../../migrations")]
async fn reconcile_finalizes_exited_orphan(pool: PgPool) {
    let fx = seed(&pool, "RE-RC1").await;
    let executor = FakeExecutor::new(Behavior::Exit(0));
    let cfg = cfg();

    claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();
    // 伪装成上一个 worker 的孤儿（新 worker 不认识 lease_owner）。
    sqlx::query("UPDATE delivery.task_run SET lease_owner = 'ai-pm-worker-old' WHERE id = $1")
        .bind(fx.run_id)
        .execute(&pool)
        .await
        .unwrap();

    let handled = reconcile_orphans(&pool, &executor, &cfg).await.unwrap();
    assert_eq!(handled, 1);
    assert_eq!(run_status(&pool, fx.run_id).await, "succeeded");
}

/// reconcile：孤儿仍存活 → 保留（租约过期后由回收路径接管）。
#[sqlx::test(migrations = "../../migrations")]
async fn reconcile_keeps_alive_orphans_untouched(pool: PgPool) {
    let fx = seed(&pool, "RE-RC2").await;
    let executor = FakeExecutor::new(Behavior::Hang);
    let cfg = cfg();

    claim_and_start(&pool, &executor, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap();
    sqlx::query("UPDATE delivery.task_run SET lease_owner = 'ai-pm-worker-old' WHERE id = $1")
        .bind(fx.run_id)
        .execute(&pool)
        .await
        .unwrap();

    let handled = reconcile_orphans(&pool, &executor, &cfg).await.unwrap();
    assert_eq!(handled, 0);
    assert_eq!(run_status(&pool, fx.run_id).await, "running");
    assert_eq!(executor.terminated_count(), 0);
}

/// 领取侧回归：Model B（claim_once）在无执行器场景下的 Skipped/Cancelled 语义
/// 不受 D3 改动影响（B3-1 释放路径）。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_once_regression_unstarted_lease_expiry_requeues_same_run(pool: PgPool) {
    let fx = seed(&pool, "RE-RC3").await;
    let cfg = cfg();
    // 直接领取一次（无人启动），把租约拨过期 → B3-1 重领同一 Run。
    let first = claim_once(&pool, "w1", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .expect("candidate exists");
    assert_eq!(first.kind, ClaimKind::Claimed);
    sqlx::query(
        "UPDATE delivery.task_run \
            SET lease_expires_at = now() - interval '1 second' \
          WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    let second = claim_once(&pool, "w2", &cfg, DispatchMode::On)
        .await
        .unwrap()
        .expect("reclaim candidate exists");
    assert_eq!(second.run_id, fx.run_id, "B3-1 must reuse the same run");
}
