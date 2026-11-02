//! `ai-pm-worker`: claims `ai_analysis` jobs and runs them through the
//! end-to-end orchestrator (Task 14). One job at a time, 60ms poll
//! between empty claims.

use std::sync::Arc;

use pm_ai::model::AnalysisModel;
use pm_ai::orchestrator::{HandleOutcome, Orchestrator};
use pm_ai::FakeModel;
use pm_application::project_import::{import_id_for_event, run_import_job};
use pm_application::{PgCommandExecutor, PolicyEngine};
use pm_infrastructure::job_queue::{self, ClaimedJob};
use pm_infrastructure::outbox_handlers::register_all_handlers;
use pm_infrastructure::outbox_worker::OutboxWorker;
use sqlx::PgPool;
use tracing_subscriber::EnvFilter;

/// Attempts ceiling / backoff base handed to `job_queue::fail`; the same
/// values the orchestrator uses when it fails a job itself.
const MAX_JOB_ATTEMPTS: i32 = 5;
const JOB_BACKOFF_BASE_SECS: i64 = 5;
/// Lease length for a claimed job, in seconds. One hour, matching the
/// Task 10 claim hardening: a real model analysis runs far longer than a
/// minute, and an expired lease would let another worker double-claim a
/// live analysis.
const LEASE_SECS: i64 = 3600;
/// Idle poll interval.
const POLL_INTERVAL_MS: u64 = 60;
/// Default on-disk policy — the red-line single source of truth
/// (global constraint: `policies/ai-pm-ai-lead-v2.yaml`). Relative to the
/// working directory the worker is started from (repo root in dev).
const DEFAULT_POLICY_FILE: &str = "policies/ai-pm-ai-lead-v2.yaml";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("database pool");

    let policy_path =
        std::env::var("AI_PM_POLICY_FILE").unwrap_or_else(|_| DEFAULT_POLICY_FILE.to_string());
    let policy_yaml = std::fs::read_to_string(&policy_path).unwrap_or_else(|e| {
        panic!("policy file `{policy_path}` unreadable: {e}; set AI_PM_POLICY_FILE or start from the repo root")
    });
    let policy = Arc::new(
        PolicyEngine::from_yaml(&policy_yaml)
            .expect("policy yaml must parse (it is the same file the tests pin)"),
    );
    let executor = Arc::new(PgCommandExecutor::new(pool.clone(), policy.clone()));
    // TODO(Task 17): replace the FakeModel with the OpenAI-compatible
    // adapter behind the same `Arc<dyn AnalysisModel>` port.
    let model: Arc<dyn AnalysisModel> = Arc::new(FakeModel::from_fixtures());
    let orchestrator = Orchestrator {
        pool: pool.clone(),
        model,
        policy,
        executor,
    };

    let worker_id = format!("ai-pm-worker-{}", uuid::Uuid::new_v4());
    tracing::info!(service = "ai-pm-worker", %worker_id, "started");

    // 第二条循环：冷启动 `repo_import` job，与 `ai_analysis` 互不阻塞。
    let repo_pool = pool.clone();
    let repo_worker_id = worker_id.clone();
    tokio::spawn(async move {
        loop {
            match job_queue::claim(&repo_pool, "repo_import", &repo_worker_id, LEASE_SECS).await {
                Some(job) => handle_repo_job(&repo_pool, &job).await,
                None => {
                    tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await
                }
            }
        }
    });

    // 第三条循环：outbox 事件分发（spec §20）。**统一 Handler 装配**
    // （T-061 §6.2.1）：本 worker 与 `pm-worker` 共享同一张 outbox 表，注册
    // 集合必须一致——否则零 Handler 实例会把可处理事件误判为死信。
    // 装配集合：TaskReady → 创建 queued Run；DeliverySubmitted → D5 拍①；
    // RunCompleted → 记审计 + 生成执行完成通知（记录型）。
    // 其余未注册事件明确死信、可 replay，绝不假装消费。
    let outbox_pool = pool.clone();
    let outbox_worker_id = worker_id.clone();
    tokio::spawn(async move {
        let worker = register_all_handlers(
            OutboxWorker::new(outbox_pool.clone(), outbox_worker_id),
            &outbox_pool,
        );
        // 目录一致性校验：不一致即 panic（fail fast）。
        worker.assert_catalog_consistent();
        worker.run().await
    });

    // 第四条循环：Run 调度与执行（Slice D1/D3）。R4：开关只控制**领取**新 Run；
    // 租约过期回收在本循环内持续运行，不因调度关闭而停摆。执行器由
    // `AI_PM_RUN_EXECUTOR` 选择（codex/mock → CodexExecutor 适配器；缺省 noop →
    // 领取即 start 失败退避，不伪造执行成功）。
    let dispatch_pool = pool.clone();
    let dispatch_worker_id = worker_id.clone();
    tokio::spawn(async move {
        let cfg = pm_application::run_dispatch::RunDispatchConfig::from_env();
        let mode = pm_application::run_dispatch::DispatchMode::from_env();
        let run_executor: Arc<dyn pm_application::run_dispatch::RunExecutor> =
            match std::env::var("AI_PM_RUN_EXECUTOR").as_deref() {
                Ok("codex") | Ok("mock") => {
                    Arc::new(pm_infrastructure::codex_executor::CodexExecutor::from_env(
                        dispatch_pool.clone(),
                    ))
                }
                _ => Arc::new(pm_application::run_dispatch::NoopExecutor),
            };
        // 启动时一次孤儿 reconcile（T-030 §3 异常路径②）；失败只告警。
        match pm_application::run_executor::reconcile_orphans(&dispatch_pool, run_executor.as_ref(), &cfg)
            .await
        {
            Ok(n) if n > 0 => tracing::info!(reconciled = n, "orphan runs reconciled at startup"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "orphan reconcile failed; relying on lease recovery"),
        }
        tracing::info!(mode = ?mode, lease_secs = cfg.lease_secs, executor = run_executor.kind(), "run dispatch loop started");
        loop {
            match pm_application::run_executor::claim_and_start(
                &dispatch_pool,
                run_executor.as_ref(),
                &dispatch_worker_id,
                &cfg,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "claim_and_start failed; backing off");
                }
            }
            match pm_application::run_executor::poll_active_runs(
                &dispatch_pool,
                run_executor.as_ref(),
                &dispatch_worker_id,
                &cfg,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "poll_active_runs failed; backing off");
                }
            }
            // T-032 D3：终态僵尸进程兜底（终止顺序反转引入的窗口——Run 已终态
            // 但 terminate 失败，reconcile_orphans 不覆盖终态行）。
            if let Err(e) =
                pm_application::run_executor::cleanup_finished_processes(&dispatch_pool, run_executor.as_ref())
                    .await
            {
                tracing::warn!(error = %e, "finished-run cleanup failed");
            }
            tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        }
    });

    // 第五条循环：D5 review 领取拍（确定性校验管线 → findings/verdict → 推进任务）。
    // 与失败判定作业并列，共享事务模板但不共享错误处理（§6.2）。
    let review_pool = pool.clone();
    tokio::spawn(async move {
        let outbox: Arc<dyn pm_application::outbox::OutboxAppend> =
            Arc::new(pm_application::outbox::PgOutboxAppend);
        pm_application::review::run_review_loop(review_pool, outbox).await
    });

    // 第六条循环：D5 失败判定作业（B4-A~D）。扫描谓词强制 run_kind='delivery'，
    // 对 review run 零写入路径（§6.2 结构性隔离）。
    let judge_pool = pool.clone();
    tokio::spawn(async move {
        loop {
            let report = pm_application::review::judge_failures_once(&judge_pool).await;
            if report.judged > 0 {
                tracing::info!(
                    judged = report.judged,
                    retries = report.retries,
                    task_failures = report.task_failures,
                    "failure judge step"
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        }
    });

    loop {
        match job_queue::claim(&pool, "ai_analysis", &worker_id, LEASE_SECS).await {
            Some(job) => handle_job(&orchestrator, &job).await,
            None => tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await,
        }
    }
}

async fn handle_repo_job(pool: &PgPool, job: &ClaimedJob) {
    let import_id = match import_id_for_event(pool, job.event_id).await {
        Ok(Some(import_id)) => import_id,
        other => {
            tracing::error!(job_id = %job.job_id, ?other, "repo import job has no import id");
            let _ = job_queue::fail(
                pool,
                job.job_id,
                "repo import job has no import id",
                MAX_JOB_ATTEMPTS,
                JOB_BACKOFF_BASE_SECS,
            )
            .await;
            return;
        }
    };
    let (model, model_name) = pm_ai::repo_model::model_from_llm_provider(pool, job.project_id).await;
    if let Err(e) = sqlx::query("UPDATE integration.repo_import SET model_name = $2 WHERE id = $1")
        .bind(import_id)
        .bind(&model_name)
        .execute(pool)
        .await
    {
        tracing::error!(job_id = %job.job_id, error = %e, "model_name write failed");
    }
    match run_import_job(pool, model.as_ref(), import_id).await {
        Ok(()) => {
            if let Err(e) = job_queue::complete(pool, job.job_id).await {
                tracing::error!(job_id = %job.job_id, error = %e, "job complete failed");
            }
        }
        Err(e) => {
            tracing::warn!(job_id = %job.job_id, error = %e, "repo import job failed, backing off");
            if let Err(e) = job_queue::fail(
                pool,
                job.job_id,
                &e.to_string(),
                MAX_JOB_ATTEMPTS,
                JOB_BACKOFF_BASE_SECS,
            )
            .await
            {
                tracing::error!(job_id = %job.job_id, error = %e, "job fail failed");
            }
        }
    }
}

/// One claimed job: `Completed` (including the idempotent already-done
/// redelivery path) completes the job; `RetryScheduled` means the
/// orchestrator already failed the job with backoff — touching it here
/// (a `complete`) would erase the scheduled retry; a `Model`/`Db` error
/// fails it with backoff so the queue retries.
async fn handle_job(orchestrator: &Orchestrator, job: &ClaimedJob) {
    match orchestrator.handle_ai_job(job).await {
        Ok(HandleOutcome::Completed) => {
            if let Err(e) = job_queue::complete(&orchestrator.pool, job.job_id).await {
                tracing::error!(job_id = %job.job_id, error = %e, "job complete failed");
            }
        }
        Ok(HandleOutcome::RetryScheduled) => {
            tracing::info!(
                job_id = %job.job_id,
                "retry already scheduled by the orchestrator; leaving the job queued"
            );
        }
        Err(e) => {
            tracing::warn!(job_id = %job.job_id, error = %e, "job failed, backing off");
            if let Err(e) = job_queue::fail(
                &orchestrator.pool,
                job.job_id,
                &e.to_string(),
                MAX_JOB_ATTEMPTS,
                JOB_BACKOFF_BASE_SECS,
            )
            .await
            {
                tracing::error!(job_id = %job.job_id, error = %e, "job fail failed");
            }
        }
    }
}
