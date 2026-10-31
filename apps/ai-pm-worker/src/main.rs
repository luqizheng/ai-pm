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
use pm_infrastructure::outbox_handlers::TaskReadyHandler;
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

    // 第三条循环：outbox 事件分发（spec §20）。只注册 TaskReady → 创建 queued
    // Run；未注册事件（如 DeliverySubmitted，开关默认关闭）明确死信、可 replay，
    // 绝不假装消费。
    let outbox_pool = pool.clone();
    let outbox_worker_id = worker_id.clone();
    tokio::spawn(async move {
        OutboxWorker::new(outbox_pool.clone(), outbox_worker_id)
            .with_handler("TaskReady", Arc::new(TaskReadyHandler::new(outbox_pool)))
            .run()
            .await
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
