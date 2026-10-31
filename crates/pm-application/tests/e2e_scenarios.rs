//! Task 18 — end-to-end scenario acceptance suite (主稿 §17.1, the M1 rows).
//!
//! Ten scenarios over the real Postgres and the real main chain. Every test
//! is self-contained: the shared fixture world (ids matching the FakeModel's
//! hard-coded proposal targets) is seeded, events enter through the real
//! [`submit_event`] ingestion, and processing goes through a faithful worker
//! loop — `job_queue::claim` → `Orchestrator::handle_ai_job` →
//! `complete` / `fail` — run to quiescence.
//!
//! Scenario 6 (change_policy) uses a local minimal `impl AnalysisModel`
//! because FakeModel never proposes policy changes (controller ruling:
//! FakeModel stays untouched); every other scenario drives the fixed
//! FakeModel fixtures.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pm_ai::model::{AnalysisContext, AnalysisModel};
use pm_ai::orchestrator::{HandleOutcome, Orchestrator, OrchestratorError};
use pm_ai::skill_router::SkillId;
use pm_ai::{FakeModel, ModelError, context as ai_context};
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::ingestion::{SubmitEventRequest, SubmitOutcome, submit_event};
use pm_application::supervision::{
    DecideRequest, Decision, DecisionOutcome, VetoOutcome, VetoRequest, decide_approval, freeze,
    unfreeze, veto,
};
use pm_application::{
    ActorKind, CommandError, CommandSource, ExecutionContext, PgCommandExecutor, PolicyEngine,
    ReadError, task_context,
};
use pm_domain::{
    AnalysisResult, Confidence, Finding, Proposal, ProjectCommand, RiskLevel, TaskStatus,
};
use pm_infrastructure::job_queue;
use serde_json::{Value, json};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixture world (copied from the Task 15 suite's conventions: the seeded rows
// reuse the FakeModel's hard-coded ids so the fixture proposals validate).
// ---------------------------------------------------------------------------

/// FakeModel's embedded project id (mirrors the fixture string).
fn project_id() -> Uuid {
    "11111111-1111-4111-8111-111111111111"
        .parse()
        .expect("fixture project id must parse")
}
/// FakeModel's `UpdateTaskStatus` target at `expected_revision: 7`.
fn task_102_id() -> Uuid {
    "22222222-2222-4222-8222-222222222222"
        .parse()
        .expect("fixture task id must parse")
}
/// FakeModel's `AdjustMilestonePlan` / `SignAcceptance` target at
/// `expected_revision: 4`.
fn milestone_id() -> Uuid {
    "33333333-3333-4333-8333-333333333333"
        .parse()
        .expect("fixture milestone id must parse")
}

/// The milestone state the fixture proposes to move to, whole-second.
const MOVED_PLANNED_AT: &str = "2026-11-06T09:00:00Z";
/// The milestone plan the seed starts from, whole-second.
const SEEDED_PLANNED_AT: &str = "2026-11-03T09:00:00Z";

fn moved_planned_at() -> DateTime<Utc> {
    MOVED_PLANNED_AT.parse().expect("fixed fixture date must parse")
}
fn seeded_planned_at() -> DateTime<Utc> {
    SEEDED_PLANNED_AT.parse().expect("fixed seed date must parse")
}

struct Seed {
    project_id: Uuid,
    /// The human supervisor exercising veto / freeze / red-line confirmation.
    human_id: Uuid,
    /// The dev-agent principal that raises events.
    dev_agent_id: Uuid,
}

/// Bootstrap-equivalent rows whose ids match the FakeModel fixtures: customer,
/// project, AI principal, human supervisor, dev agent, `ai_lead` config,
/// TASK-102 at revision 7 (`in_progress`) and the milestone at revision 4.
async fn seed_world(pool: &sqlx::PgPool) -> Seed {
    let customer_id = Uuid::new_v4();
    let ai_principal_id = Uuid::new_v4();
    let human_id = Uuid::new_v4();
    let dev_agent_id = Uuid::new_v4();

    sqlx::query("INSERT INTO core.customer (id, name) VALUES ($1, 'Acme')")
        .bind(customer_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name) \
         VALUES ($1, $2, 'FIXTURE', 'Scenario Project')",
    )
    .bind(project_id())
    .bind(customer_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'AI PM')")
        .bind(ai_principal_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', 'Supervisor')",
    )
    .bind(human_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'dev_agent', 'Dev Agent')",
    )
    .bind(dev_agent_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.project_ai_config \
         (project_id, ai_principal_id, model_provider, model_name, automation_mode, enabled) \
         VALUES ($1, $2, 'test', 'fake-model', 'ai_lead', true)",
    )
    .bind(project_id())
    .bind(ai_principal_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO delivery.milestone (id, project_id, name, status, revision, planned_at) \
         VALUES ($1, $2, 'Design freeze', 'planned', 4, $3)",
    )
    .bind(milestone_id())
    .bind(project_id())
    .bind(seeded_planned_at())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task \
         (id, project_id, milestone_id, title, status, revision) \
         VALUES ($1, $2, $3, 'TASK-102 implement cache layer', 'in_progress', 7)",
    )
    .bind(task_102_id())
    .bind(project_id())
    .bind(milestone_id())
    .execute(pool)
    .await
    .unwrap();

    Seed {
        project_id: project_id(),
        human_id,
        dev_agent_id,
    }
}

/// The `cache_overlap` inbound payload: fixture marker plus the resolved
/// delivery refs the context builder must find in this project.
fn cache_overlap_payload() -> Value {
    json!({
        "fixture": "cache_overlap",
        "task_ref": task_102_id().to_string(),
        "milestone_ref": milestone_id().to_string(),
    })
}

/// The `redline_acceptance` inbound payload.
fn redline_payload() -> Value {
    json!({
        "fixture": "redline_acceptance",
        "milestone_ref": milestone_id().to_string(),
    })
}

/// Enters one event through the real ingestion path (`source = 'mcp'`, a
/// fresh idempotency key) and returns its event id.
async fn submit_fixture(
    pool: &sqlx::PgPool,
    seed: &Seed,
    event_type: &str,
    payload: Value,
) -> Uuid {
    match submit_event(
        pool,
        SubmitEventRequest {
            project_id: seed.project_id,
            actor_id: seed.dev_agent_id,
            source: "mcp".to_string(),
            event_type: event_type.to_string(),
            idempotency_key: Uuid::new_v4().to_string(),
            payload,
        },
    )
    .await
    .unwrap()
    {
        SubmitOutcome::Accepted { event_id } => event_id,
        other => panic!("expected a fresh Accepted submission, got {other:?}"),
    }
}

/// The orchestrator under test: FakeModel behind the model port, the
/// for-tests policy, and the real Postgres executor.
fn orchestrator(pool: &sqlx::PgPool) -> Orchestrator {
    orchestrator_with_model(pool, Arc::new(FakeModel::from_fixtures()))
}

/// Same wiring with a caller-chosen model port (scenario 6's local model).
fn orchestrator_with_model(pool: &sqlx::PgPool, model: Arc<dyn AnalysisModel>) -> Orchestrator {
    let policy = Arc::new(PolicyEngine::for_tests());
    let executor = Arc::new(PgCommandExecutor::new(pool.clone(), policy.clone()));
    Orchestrator {
        pool: pool.clone(),
        model,
        policy,
        executor,
    }
}

/// The supervision-side executor. The executor is stateless over the pool +
/// policy, so any instance sees the same rows the orchestrator wrote.
fn test_executor(pool: &sqlx::PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

/// A faithful worker pass: claim → `handle_ai_job` → complete / fail, run
/// until nothing is claimable, returning how many jobs it processed. Job
/// retries use `max_attempts = 3` with a 1-second backoff base.
/// Drives at most `max_jobs` claim→handle→complete/fail cycles. The cap keeps
/// one pass equal to one job even when a failed job's short backoff has
/// already elapsed by the next iteration (slow-host reality): retries are
/// observed across separate passes, matching the per-pass assertions below.
async fn worker_loop(
    pool: &sqlx::PgPool,
    orch: &Orchestrator,
    worker_id: &str,
    max_jobs: usize,
) -> usize {
    let mut handled = 0usize;
    loop {
        if handled >= max_jobs {
            break;
        }
        let Some(job) = job_queue::claim(pool, "ai_analysis", worker_id, 60).await else {
            break;
        };
        match orch.handle_ai_job(&job).await {
            Ok(HandleOutcome::Completed) => {
                job_queue::complete(pool, job.job_id)
                    .await
                    .expect("job complete must succeed");
            }
            // The orchestrator already failed the job itself (backoff retry
            // scheduled); completing it here would erase the retry.
            Ok(HandleOutcome::RetryScheduled) => {}
            Err(OrchestratorError::Model(err)) => {
                let code = match err {
                    ModelError::Timeout => "model_timeout",
                    ModelError::Unavailable(_) => "model_unavailable",
                    ModelError::BadOutput(_) => "model_bad_output",
                };
                job_queue::fail(pool, job.job_id, code, 3, 1)
                    .await
                    .expect("job fail must succeed");
            }
            Err(err) => panic!("unexpected orchestrator error: {err}"),
        }
        handled += 1;
        let (a, s, av): (i32, String, Option<DateTime<Utc>>) = sqlx::query_as(
            "SELECT attempts, status, available_at FROM integration.job WHERE id = $1",
        )
        .bind(job.job_id)
        .fetch_one(pool)
        .await
        .unwrap();
        eprintln!(
            "[worker] claim #{handled} job={} attempts={a} status={s} available_at={av:?} now={:?}",
            job.job_id,
            Utc::now()
        );
    }
    handled
}

/// Rushes a backed-off job back into the claimable present (the suite must
/// not sleep out real exponential backoffs).
async fn fast_forward_backoff(pool: &sqlx::PgPool, job_id: Uuid) {
    sqlx::query("UPDATE integration.job SET available_at = now() WHERE id = $1 AND status = 'queued'")
        .bind(job_id)
        .execute(pool)
        .await
        .unwrap();
}

/// The single `integration.job` row of `event_id` (one event, one job).
async fn job_id_of(pool: &sqlx::PgPool, event_id: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM integration.job WHERE event_id = $1")
        .bind(event_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn scalar_i64(pool: &sqlx::PgPool, sql: &str, project_id: Uuid) -> i64 {
    sqlx::query_scalar(sql)
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Scenario 1 — idempotency: ten identical submissions, one execution.
// ---------------------------------------------------------------------------

/// The same (project, source='mcp', idempotency_key) submitted 10 times lands
/// exactly one event row and one job row; one worker pass executes the
/// fixture's three commands exactly once, and a second pass executes nothing.
#[sqlx::test(migrations = "../../migrations")]
async fn s1_ten_idempotent_submissions_execute_commands_once(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    let payload = cache_overlap_payload();

    let mut first_event_id: Option<Uuid> = None;
    for submission in 0..10 {
        let outcome = submit_event(
            &pool,
            SubmitEventRequest {
                project_id: seed.project_id,
                actor_id: seed.dev_agent_id,
                source: "mcp".to_string(),
                event_type: "technical_design_submitted".to_string(),
                idempotency_key: "design-submission-001".to_string(),
                payload: payload.clone(),
            },
        )
        .await
        .unwrap();
        if submission == 0 {
            let SubmitOutcome::Accepted { event_id } = outcome else {
                panic!("the first submission must be Accepted, got {outcome:?}");
            };
            first_event_id = Some(event_id);
        } else {
            let SubmitOutcome::Duplicate { event_id, status } = outcome else {
                panic!("submission {submission} must be an idempotent Duplicate, got {outcome:?}");
            };
            assert_eq!(event_id, first_event_id.unwrap(), "replays return the original event");
            assert_eq!(status, "received");
        }
    }

    // Exactly one event and one job survived the ten submissions.
    let (events, jobs): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM integration.inbound_event WHERE project_id = $1), \
                (SELECT count(*) FROM integration.job WHERE project_id = $1)",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 1);
    assert_eq!(jobs, 1);

    let orch = orchestrator(&pool);
    let handled = worker_loop(&pool, &orch, "worker-s1", usize::MAX).await;
    assert_eq!(handled, 1, "exactly one job was claimable");

    // The three cache_overlap commands executed once: three ai_proposal
    // audit rows, one created task, single forward moves of task/milestone.
    let audits = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND source_kind = 'ai_proposal'",
        seed.project_id,
    )
    .await;
    assert_eq!(audits, 3, "each of the three fixture commands audited exactly once");
    let tasks: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task WHERE project_id = $1")
            .bind(seed.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(tasks, 2, "TASK-102 plus the one created investigation task");
    let (task_revision, milestone_revision): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT revision FROM delivery.task WHERE id = $2), \
                (SELECT revision FROM delivery.milestone WHERE id = $3)",
    )
    .bind(seed.project_id)
    .bind(task_102_id())
    .bind(milestone_id())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(task_revision, 8, "7 seeded, bumped once to needs_info");
    assert_eq!(milestone_revision, 5, "4 seeded, bumped once to the moved plan");

    // A second worker pass finds nothing claimable and executes nothing more.
    let handled_again = worker_loop(&pool, &orch, "worker-s1", usize::MAX).await;
    assert_eq!(handled_again, 0);
    let audits_again = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND source_kind = 'ai_proposal'",
        seed.project_id,
    )
    .await;
    assert_eq!(audits_again, 3, "redelivery through the queue executes nothing twice");
}

// ---------------------------------------------------------------------------
// Scenario 2 — model_timeout: retries with backoff, terminal failure,
// the original event preserved.
// ---------------------------------------------------------------------------

/// The `model_timeout` fixture fails the run; the worker fails the job
/// (attempts+1, queued again with backoff). Each retry opens a fresh failed
/// run while the original event stays intact; at max attempts the job is
/// terminally `failed` and nothing was ever executed.
#[sqlx::test(migrations = "../../migrations")]
async fn s2_model_timeout_retries_then_fails_job_event_preserved(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    let payload = json!({
        "fixture": "model_timeout",
        "task_ref": task_102_id().to_string(),
        "milestone_ref": milestone_id().to_string(),
    });
    let event_id = submit_fixture(&pool, &seed, "technical_design_submitted", payload.clone()).await;
    let job_id = job_id_of(&pool, event_id).await;

    let orch = orchestrator(&pool);
    let handled = worker_loop(&pool, &orch, "worker-s2", 1).await;
    assert_eq!(handled, 1);

    // First failure: attempts incremented, queued again (retryable), coarse
    // error recorded — and the original event is preserved untouched.
    let (attempts, status, last_error): (i32, String, String) =
        sqlx::query_as("SELECT attempts, status, last_error FROM integration.job WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(status, "queued", "attempt 1 < max: retryable");
    assert_eq!(last_error, "model_timeout");
    let (event_status, stored_payload): (String, Value) = sqlx::query_as(
        "SELECT status, payload FROM integration.inbound_event WHERE id = $1",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_status, "received", "a model failure must not consume the event");
    assert_eq!(stored_payload, payload, "the original payload is preserved verbatim");

    // Attempts two and three: fresh failed runs per retry (run-level retries
    // are new rows), until the job goes terminally failed.
    for expected_attempts in 2..=3 {
        fast_forward_backoff(&pool, job_id).await;
        let handled = worker_loop(&pool, &orch, "worker-s2", 1).await;
        assert_eq!(handled, 1);
        let (attempts, status): (i32, String) =
            sqlx::query_as("SELECT attempts, status FROM integration.job WHERE id = $1")
                .bind(job_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(attempts, expected_attempts);
        if expected_attempts < 3 {
            assert_eq!(status, "queued", "still retryable below max_attempts");
        } else {
            assert_eq!(status, "failed", "max_attempts reached: terminal failure");
        }
    }

    // Every attempt left exactly one terminal failed run, all model_timeout.
    let (runs, failed_runs, timeout_runs): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*), \
                count(*) FILTER (WHERE status = 'failed'), \
                count(*) FILTER (WHERE failure_code = 'model_timeout') \
         FROM ai.analysis_run WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(runs, 3, "one fresh run row per attempt");
    assert_eq!(failed_runs, 3);
    assert_eq!(timeout_runs, 3);

    // Nothing ever executed: no proposals, no audit, event still received.
    let proposals =
        scalar_i64(&pool, "SELECT count(*) FROM ai.change_proposal WHERE project_id = $1", seed.project_id).await;
    assert_eq!(proposals, 0);
    let audits =
        scalar_i64(&pool, "SELECT count(*) FROM governance.audit_log WHERE project_id = $1", seed.project_id).await;
    assert_eq!(audits, 0);
    let (task_revision, milestone_revision): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT revision FROM delivery.task WHERE id = $2), \
                (SELECT revision FROM delivery.milestone WHERE id = $3)",
    )
    .bind(seed.project_id)
    .bind(task_102_id())
    .bind(milestone_id())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(task_revision, 7, "no silent state change under a failing model");
    assert_eq!(milestone_revision, 4);

    // A terminal job is never handed out again.
    fast_forward_backoff(&pool, job_id).await;
    let handled = worker_loop(&pool, &orch, "worker-s2", 1).await;
    assert_eq!(handled, 0, "a failed job leaves the queue for good");
}

// ---------------------------------------------------------------------------
// Scenario 3 — concurrent revision conflict: no silent overwrite.
// ---------------------------------------------------------------------------

/// Two dev agents fire the same `UpdateTaskStatus` with the same
/// `expected_revision` through the executor; sequentially the second one must
/// be rejected with `RevisionConflict` and the row keeps the first agent's
/// state at its single forward revision.
#[sqlx::test(migrations = "../../migrations")]
async fn s3_second_agent_on_same_expected_revision_gets_revision_conflict(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;

    // The second agent: a real principal so its audit rows satisfy FKs.
    let agent_b = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'dev_agent', 'Dev Agent B')",
    )
    .bind(agent_b)
    .execute(&pool)
    .await
    .unwrap();

    let executor = test_executor(&pool);
    let direct = |actor_id: Uuid| ExecutionContext {
        project_id: seed.project_id,
        actor_id,
        actor_kind: ActorKind::DevAgent,
        source: CommandSource::Bootstrap,
    };

    // Agent A moves TASK-102 from revision 7 to needs_info.
    let first = executor
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: task_102_id(),
                expected_revision: 7,
                new_status: TaskStatus::AwaitingDecision,
            },
            direct(seed.dev_agent_id),
        )
        .await
        .expect("the first writer at revision 7 must succeed");
    assert_eq!(first.resulting_revision, 8);

    // Agent B still believes the revision is 7 (and wants done): rejected.
    let err = executor
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: task_102_id(),
                expected_revision: 7,
                new_status: TaskStatus::Completed,
            },
            direct(agent_b),
        )
        .await
        .expect_err("a stale expected_revision must be rejected");
    assert_eq!(
        match &err {
            CommandError::RevisionConflict { expected, actual } => (*expected, *actual),
            other => panic!("expected RevisionConflict, got {other:?}"),
        },
        (7, 8),
        "the conflict must name both revisions"
    );

    // No silent overwrite: A's state stands, exactly one revision forward,
    // exactly one audit row for the task.
    let (status, revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.task WHERE id = $1")
            .bind(task_102_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "awaiting_decision", "agent B must not have flipped it to completed");
    assert_eq!(revision, 8);
    let audits: i64 =
        sqlx::query_scalar("SELECT count(*) FROM governance.audit_log WHERE target_id = $1")
            .bind(task_102_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audits, 1, "only the winning write is on the audit trail");

    // And the losing agent retries on fresh information: now it succeeds,
    // proving the conflict was a guard, not a blacklisting.
    let retried = executor
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id: task_102_id(),
                expected_revision: 8,
                new_status: TaskStatus::Completed,
            },
            direct(agent_b),
        )
        .await
        .expect("a retry on the current revision must succeed");
    assert_eq!(retried.resulting_revision, 9);
}

// ---------------------------------------------------------------------------
// Scenario 4 — AI change auto-applied, then vetoed within the window:
// milestone data restored, audit chain closed.
// ---------------------------------------------------------------------------

/// The `cache_overlap` chain auto-applies its three proposals (with audit and
/// notifications); vetoing the `adjust_milestone_plan` proposal rolls it back
/// through the real snapshot compensation — the milestone plan is restored
/// and the audit chain closes (rollback audit pointing at the original).
#[sqlx::test(migrations = "../../migrations")]
async fn s4_auto_applied_milestone_change_veto_restores_data_and_closes_audit_chain(
    pool: sqlx::PgPool,
) {
    let seed = seed_world(&pool).await;
    let event_id =
        submit_fixture(&pool, &seed, "technical_design_submitted", cache_overlap_payload()).await;
    let orch = orchestrator(&pool);
    let handled = worker_loop(&pool, &orch, "worker-s4", usize::MAX).await;
    assert_eq!(handled, 1, "premise: the chain ran once");
    let _ = event_id;

    // Pre-veto state: milestone moved (4 -> 5), all three executions
    // notified with a veto window.
    let (planned_at, revision): (Option<DateTime<Utc>>, i64) =
        sqlx::query_as("SELECT planned_at, revision FROM delivery.milestone WHERE id = $1")
            .bind(milestone_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(planned_at, Some(moved_planned_at()), "the AI move took effect");
    assert_eq!(revision, 5);
    let applied = scalar_i64(
        &pool,
        "SELECT count(*) FROM integration.notification \
         WHERE project_id = $1 AND kind = 'change_applied'",
        seed.project_id,
    )
    .await;
    assert_eq!(applied, 3, "one change_applied notification per executed proposal");

    let (proposal_id, executed_audit_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, executed_audit_id FROM ai.change_proposal \
         WHERE project_id = $1 AND command_type = 'adjust_milestone_plan'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM ai.change_proposal WHERE id = $1")
            .bind(proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "executed"
    );

    // The human vetoes inside the window: rolled back.
    let outcome = veto(
        &pool,
        &test_executor(&pool),
        VetoRequest {
            project_id: seed.project_id,
            proposal_id,
            actor_id: seed.human_id,
            reason: "排期不能动，回滚".to_string(),
        },
    )
    .await
    .unwrap();
    let VetoOutcome::RolledBack { new_audit_id } = outcome else {
        panic!("expected RolledBack, got {outcome:?}");
    };

    // The milestone data is restored: seeded plan back, revision only forward.
    let (planned_at, revision): (Option<DateTime<Utc>>, i64) =
        sqlx::query_as("SELECT planned_at, revision FROM delivery.milestone WHERE id = $1")
            .bind(milestone_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(planned_at, Some(seeded_planned_at()), "the original plan is back");
    assert_ne!(planned_at, Some(moved_planned_at()));
    assert_eq!(revision, 6, "4 seeded -> 5 executed -> 6 restored");

    // The audit chain closes both ways: the original execution points at its
    // rollback, and the rollback audit is a `rollback_command` whose source
    // is the audit it undoes.
    let compensated: Uuid = sqlx::query_scalar(
        "SELECT compensated_command_id FROM governance.audit_log WHERE id = $1",
    )
    .bind(executed_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(compensated, new_audit_id);
    let (action, source_kind, source_ref): (String, String, Uuid) = sqlx::query_as(
        "SELECT action, source_kind, source_ref FROM governance.audit_log WHERE id = $1",
    )
    .bind(new_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "rollback_command");
    assert_eq!(source_kind, "rollback");
    assert_eq!(source_ref, executed_audit_id, "the rollback names the audit it undoes");

    // The veto reached the feed with the rollback anchor.
    let (kind, payload): (String, Value) = sqlx::query_as(
        "SELECT kind, payload FROM integration.notification \
         WHERE project_id = $1 AND kind = 'veto_applied'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "veto_applied");
    assert_eq!(payload["proposal_id"], json!(proposal_id));
    assert_eq!(payload["rollback_audit_id"], json!(new_audit_id));
}

// ---------------------------------------------------------------------------
// Scenario 5 — red-line acceptance: intercepted into a pending pre-confirmation,
// the human approval executes it exactly once.
// ---------------------------------------------------------------------------

/// `redline_acceptance` never executes by itself: the sign_acceptance proposal
/// parks as `redline_pending` behind a pending approval request while the
/// milestone stays untouched. The human's Approve executes it exactly once;
/// a repeat decision is `AlreadyDecided` and executes nothing more.
#[sqlx::test(migrations = "../../migrations")]
async fn s5_redline_becomes_pending_approval_and_approve_executes_once(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    submit_fixture(&pool, &seed, "milestone_check", redline_payload()).await;
    let orch = orchestrator(&pool);
    let handled = worker_loop(&pool, &orch, "worker-s5", usize::MAX).await;
    assert_eq!(handled, 1);

    // Parked, not executed: milestone untouched, approval pending.
    let (milestone_status, milestone_revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.milestone WHERE id = $1")
            .bind(milestone_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(milestone_status, "planned");
    assert_eq!(milestone_revision, 4);
    let (approval_id, proposal_id, required_role): (Uuid, Uuid, String) = sqlx::query_as(
        "SELECT id, proposal_id, required_role FROM governance.approval_request \
         WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(required_role, "authorized_acceptor");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM governance.approval_request WHERE id = $1")
            .bind(approval_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "pending"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM ai.change_proposal WHERE id = $1")
            .bind(proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "redline_pending"
    );
    let redline_notifications = scalar_i64(
        &pool,
        "SELECT count(*) FROM integration.notification \
         WHERE project_id = $1 AND kind = 'redline_pending'",
        seed.project_id,
    )
    .await;
    assert_eq!(redline_notifications, 1);

    // The human approves: the red line executes through the human channel.
    let outcome = decide_approval(
        &pool,
        &test_executor(&pool),
        DecideRequest {
            approval_id,
            actor_id: seed.human_id,
            decision: Decision::Approve,
            reason: "验收确认".to_string(),
        },
    )
    .await
    .unwrap();
    let DecisionOutcome::Executed { receipt } = outcome else {
        panic!("expected Executed, got {outcome:?}");
    };
    assert_eq!(receipt.command_kind, "sign_acceptance");
    assert_eq!(receipt.resulting_revision, 5);

    let (milestone_status, milestone_revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.milestone WHERE id = $1")
            .bind(milestone_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(milestone_status, "accepted");
    assert_eq!(milestone_revision, 5, "executed exactly once");
    let executions = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND source_kind = 'redline_approval'",
        seed.project_id,
    )
    .await;
    assert_eq!(executions, 1);
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT executed_audit_id FROM ai.change_proposal WHERE id = $1"
        )
        .bind(proposal_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        receipt.audit_id,
        "the proposal carries the human-channel execution"
    );

    // The repeat decision cannot execute anything again.
    let err = decide_approval(
        &pool,
        &test_executor(&pool),
        DecideRequest {
            approval_id,
            actor_id: seed.human_id,
            decision: Decision::Approve,
            reason: "duplicate click".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, pm_application::SupervisionError::Command(CommandError::AlreadyDecided { .. })),
        "expected AlreadyDecided, got {err:?}"
    );
    let executions_again = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND source_kind = 'redline_approval'",
        seed.project_id,
    )
    .await;
    assert_eq!(executions_again, 1, "no second execution");
    let revision_again: i64 =
        sqlx::query_scalar("SELECT revision FROM delivery.milestone WHERE id = $1")
            .bind(milestone_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revision_again, 5, "no second revision bump");
}

// ---------------------------------------------------------------------------
// Scenario 6 — change_policy: no handler, never executed, on record as failed.
// ---------------------------------------------------------------------------

/// A minimal local model (FakeModel never proposes policy changes) makes the
/// AI attempt `ChangePolicy`. The executor has no handler, so the orchestrator
/// fails the proposal at validation: the `change_policy` proposal row exists,
/// no audit was written, no approval/notification issued, the project config
/// is untouched — the attempt is on record without any side effect.
#[sqlx::test(migrations = "../../migrations")]
async fn s6_change_policy_proposal_is_never_executed_and_leaves_a_failed_record(
    pool: sqlx::PgPool,
) {
    struct ChangePolicyModel {
        project_id: Uuid,
    }

    #[async_trait]
    impl AnalysisModel for ChangePolicyModel {
        async fn analyze(
            &self,
            _context: &AnalysisContext,
            _skills: &[SkillId],
        ) -> Result<AnalysisResult, ModelError> {
            Ok(AnalysisResult {
                summary: "AI 试图直接修改项目自动化政策（越权尝试）".to_string(),
                findings: vec![Finding {
                    skill: "architecture_consistency".to_string(),
                    severity: "warning".to_string(),
                    description: "模型提议修改自动化政策，超出 AI 可执行边界。".to_string(),
                    evidence_refs: Vec::new(),
                }],
                proposals: vec![Proposal {
                    command: ProjectCommand::ChangePolicy {
                        project_id: self.project_id,
                        new_policy: json!({ "automation_mode": "controlled" }),
                    },
                    rationale: "试图绕过人工控制直接改政策——必须被拦截。".to_string(),
                    risk_level: RiskLevel::High,
                    confidence: Confidence::High,
                    evidence: Vec::new(),
                }],
            })
        }
    }

    let seed = seed_world(&pool).await;

    // Layer 1 (executor): the handler map simply has no change_policy entry.
    assert!(
        !test_executor(&pool).has_handler("change_policy"),
        "M1 must not register a change_policy handler"
    );
    // And a direct execution attempt is NotHandled — the belt to the
    // orchestrator's braces.
    let err = test_executor(&pool)
        .execute(
            ProjectCommand::ChangePolicy {
                project_id: seed.project_id,
                new_policy: json!({ "automation_mode": "controlled" }),
            },
            ExecutionContext {
                project_id: seed.project_id,
                actor_id: seed.human_id,
                actor_kind: ActorKind::Human,
                source: CommandSource::Bootstrap,
            },
        )
        .await
        .expect_err("change_policy must never be handled");
    assert!(
        matches!(err, CommandError::NotHandled { ref command } if command == "change_policy"),
        "expected NotHandled, got {err:?}"
    );

    // Layer 2 (orchestrator validation): the model's proposal becomes a
    // `failed` proposal row and nothing else.
    submit_fixture(&pool, &seed, "technical_design_submitted", json!({ "trigger": "policy" }))
        .await;
    let orch = orchestrator_with_model(&pool, Arc::new(ChangePolicyModel { project_id: seed.project_id }));
    let handled = worker_loop(&pool, &orch, "worker-s6", usize::MAX).await;
    assert_eq!(handled, 1);

    let (command_type, status): (String, String) = sqlx::query_as(
        "SELECT command_type, status FROM ai.change_proposal WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(command_type, "change_policy");
    assert_eq!(status, "failed", "validation failure, not executed");

    // No side effects anywhere: no audit write, no approval, no notification,
    // the policy config untouched.
    let audits =
        scalar_i64(&pool, "SELECT count(*) FROM governance.audit_log WHERE project_id = $1", seed.project_id).await;
    assert_eq!(audits, 0, "an unhandleable command must never reach the audit trail");
    let approvals = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.approval_request WHERE project_id = $1",
        seed.project_id,
    )
    .await;
    assert_eq!(approvals, 0);
    let notifications = scalar_i64(
        &pool,
        "SELECT count(*) FROM integration.notification WHERE project_id = $1",
        seed.project_id,
    )
    .await;
    assert_eq!(notifications, 0);
    let (policy_version, automation_mode): (i32, String) = sqlx::query_as(
        "SELECT policy_version, automation_mode FROM ai.project_ai_config WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(policy_version, 1, "the policy was never changed");
    assert_eq!(automation_mode, "ai_lead", "the mode was never flipped");

    // The run completed; with no execution the event needs a human look.
    let event_status: String = sqlx::query_scalar(
        "SELECT status FROM integration.inbound_event \
         WHERE project_id = $1 AND event_type = 'technical_design_submitted'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_status, "needs_clarification");
}

// ---------------------------------------------------------------------------
// Scenario 7 — vetoing an already-rolled-back operation: AlreadyCompensated.
// ---------------------------------------------------------------------------

/// The first veto of the executed `update_task_status` proposal rolls it back
/// (task restored, revision forward-only); the second veto on the same
/// proposal is the idempotent `AlreadyCompensated` outcome — recorded as an
/// intervention, but no second rollback is written and the data stays put.
#[sqlx::test(migrations = "../../migrations")]
async fn s7_second_veto_on_rolled_back_proposal_is_already_compensated(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    submit_fixture(&pool, &seed, "technical_design_submitted", cache_overlap_payload()).await;
    let orch = orchestrator(&pool);
    let handled = worker_loop(&pool, &orch, "worker-s7", usize::MAX).await;
    assert_eq!(handled, 1);

    let (proposal_id, executed_audit_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, executed_audit_id FROM ai.change_proposal \
         WHERE project_id = $1 AND command_type = 'update_task_status'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let first = veto(
        &pool,
        &test_executor(&pool),
        VetoRequest {
            project_id: seed.project_id,
            proposal_id,
            actor_id: seed.human_id,
            reason: "第一次否决：方向错误".to_string(),
        },
    )
    .await
    .unwrap();
    let VetoOutcome::RolledBack { new_audit_id } = first else {
        panic!("expected RolledBack, got {first:?}");
    };
    // The task is restored: back to in_progress, revision forward-only.
    let (status, revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.task WHERE id = $1")
            .bind(task_102_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "in_progress");
    assert_eq!(revision, 9, "7 seeded -> 8 executed -> 9 restored");

    let audits_after_first = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.audit_log WHERE project_id = $1",
        seed.project_id,
    )
    .await;
    assert_eq!(audits_after_first, 4, "3 executions + 1 rollback");

    // The second veto on the same proposal: idempotent AlreadyCompensated.
    let second = veto(
        &pool,
        &test_executor(&pool),
        VetoRequest {
            project_id: seed.project_id,
            proposal_id,
            actor_id: seed.human_id,
            reason: "重复否决（幂等）".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(second, VetoOutcome::AlreadyCompensated);

    // Nothing moved and nothing new was written by the repeat.
    let (status, revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.task WHERE id = $1")
            .bind(task_102_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "in_progress");
    assert_eq!(revision, 9);
    let audits_after_second = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.audit_log WHERE project_id = $1",
        seed.project_id,
    )
    .await;
    assert_eq!(audits_after_second, audits_after_first, "no second rollback audit");
    let rollbacks: Uuid = sqlx::query_scalar(
        "SELECT compensated_command_id FROM governance.audit_log WHERE id = $1",
    )
    .bind(executed_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rollbacks, new_audit_id, "the chain still points at the first rollback");
    let interventions = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.human_intervention WHERE project_id = $1",
        seed.project_id,
    )
    .await;
    assert_eq!(interventions, 2, "each veto records itself, executed or not");
}

// ---------------------------------------------------------------------------
// Scenario 8 — the freeze brake: jobs stop being claimed, nothing is lost,
// unfreeze resumes the very same job.
// ---------------------------------------------------------------------------

/// freeze stops `claim` from handing out the queued job while the event stays
/// intact (`received`, payload preserved); unfreeze hands out the same event's
/// job again and the worker completes the whole chain — the event is applied,
/// never lost.
#[sqlx::test(migrations = "../../migrations")]
async fn s8_freeze_stops_claiming_unfreeze_resumes_and_no_event_is_lost(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    let payload = cache_overlap_payload();
    let event_id =
        submit_fixture(&pool, &seed, "technical_design_submitted", payload.clone()).await;
    let job_id = job_id_of(&pool, event_id).await;

    // Brake on.
    freeze(&pool, seed.project_id, seed.human_id, "发布前刹车").await.unwrap();

    let orch = orchestrator(&pool);
    let handled = worker_loop(&pool, &orch, "worker-s8", usize::MAX).await;
    assert_eq!(handled, 0, "a frozen project must hand out no jobs");
    let job_status: String =
        sqlx::query_scalar("SELECT status FROM integration.job WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job_status, "queued", "the job waits, not lost, not failed");
    let (event_status, stored_payload): (String, Value) = sqlx::query_as(
        "SELECT status, payload FROM integration.inbound_event WHERE id = $1",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_status, "received", "the event is preserved during the freeze");
    assert_eq!(stored_payload, payload);
    let audits =
        scalar_i64(&pool, "SELECT count(*) FROM governance.audit_log WHERE project_id = $1", seed.project_id).await;
    assert_eq!(audits, 0, "nothing executed while frozen");

    // Brake off: the very same event's job is handed out again and the
    // claimed job completes through the real handler.
    unfreeze(&pool, seed.project_id, seed.human_id, "恢复 AI 领航").await.unwrap();
    let claimed = job_queue::claim(&pool, "ai_analysis", "worker-s8", 60)
        .await
        .expect("after unfreeze the job must be claimable again");
    assert_eq!(claimed.event_id, event_id, "the same event's job resumes");
    let outcome = orch
        .handle_ai_job(&claimed)
        .await
        .expect("the resumed chain must complete");
    assert_eq!(outcome, HandleOutcome::Completed);
    job_queue::complete(&pool, claimed.job_id)
        .await
        .expect("job complete must succeed");

    let job_status: String =
        sqlx::query_scalar("SELECT status FROM integration.job WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job_status, "completed");
    let event_status: String =
        sqlx::query_scalar("SELECT status FROM integration.inbound_event WHERE id = $1")
            .bind(event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(event_status, "applied", "the preserved event was fully processed");
    let audits =
        scalar_i64(&pool, "SELECT count(*) FROM governance.audit_log WHERE project_id = $1", seed.project_id).await;
    assert_eq!(audits, 3);
}

// ---------------------------------------------------------------------------
// Scenario 9 — cross-project reads do not leak.
// ---------------------------------------------------------------------------

/// A task that exists in project A is invisible from project B:
/// `task_context` returns `NotFound` — indistinguishable from a task that
/// exists nowhere, so a foreign id cannot be probed.
#[sqlx::test(migrations = "../../migrations")]
async fn s9_task_context_from_wrong_project_is_not_found(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;

    // A second, unrelated project with its own world.
    let other = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "S9-OTHER".to_string(),
            project_name: "other project".to_string(),
        },
    )
    .await
    .unwrap();

    // Positive control: the right project reads its task.
    let own = task_context(&pool, seed.project_id, task_102_id())
        .await
        .expect("the owning project must read its own task");
    assert_eq!(own.task_id, task_102_id());
    assert_eq!(own.status, "in_progress");
    assert_eq!(own.revision, 7);

    // Cross-project: NotFound.
    let err = task_context(&pool, other.project_id, task_102_id())
        .await
        .expect_err("a foreign project must not see the task");
    assert!(matches!(err, ReadError::NotFound), "expected NotFound, got {err:?}");

    // Same error as a task that exists nowhere — no probing oracle.
    let missing = task_context(&pool, seed.project_id, Uuid::new_v4()).await;
    assert!(matches!(missing, Err(ReadError::NotFound)));
}

// ---------------------------------------------------------------------------
// Scenario 10 — worker crash mid-handle: the reopened run is reused,
// nothing is duplicated.
// ---------------------------------------------------------------------------

/// A worker dies after `start_run` (the run row stays `analyzing`, the lease
/// expires). The re-claimed job adopts the same run (Task 12 crash-recovery
/// ruling) instead of opening a second one, and the completed chain shows
/// exactly one run, three proposals, three audits, three notifications —
/// no duplicates of anything.
#[sqlx::test(migrations = "../../migrations")]
async fn s10_crash_mid_handle_reuses_the_open_run_without_duplicates(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    let event_id =
        submit_fixture(&pool, &seed, "technical_design_submitted", cache_overlap_payload()).await;
    let job_id = job_id_of(&pool, event_id).await;

    // Crash simulation: the worker claimed the job, opened the run, then
    // died before any chain step — the lease expires with the job still
    // `running` and the run still `analyzing`.
    let claimed = job_queue::claim(&pool, "ai_analysis", "worker-s10-crashed", 60)
        .await
        .expect("the queued job must be claimable");
    assert_eq!(claimed.event_id, event_id);
    let crashed_run_id = ai_context::start_run(&pool, claimed.event_id)
        .await
        .expect("start_run must open the first run");
    let (run_status, finished): (String, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT status, finished_at FROM ai.analysis_run WHERE id = $1",
    )
    .bind(crashed_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(run_status, "analyzing");
    assert!(finished.is_none(), "premise: the run was dropped mid-handle");

    // Recovery: expire the dead lease, re-claim, re-handle to completion.
    sqlx::query("UPDATE integration.job SET lease_until = now() - interval '1 second' WHERE id = $1")
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();
    let orch = orchestrator(&pool);
    let handled = worker_loop(&pool, &orch, "worker-s10-recovered", usize::MAX).await;
    assert_eq!(handled, 1);

    // The same run was reused, not duplicated.
    let (runs, completed_runs): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE status = 'completed') \
         FROM ai.analysis_run WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(runs, 1, "crash recovery must not open a second run row");
    assert_eq!(completed_runs, 1);
    let attempt_count: i32 =
        sqlx::query_scalar("SELECT attempt_count FROM ai.analysis_run WHERE id = $1")
            .bind(crashed_run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempt_count, 0, "the adopted run is the first attempt, finished");

    // Exactly-once side effects across the crash boundary.
    let (proposals, executed_proposals): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE status = 'executed') \
         FROM ai.change_proposal WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(proposals, 3);
    assert_eq!(executed_proposals, 3);
    let audits = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.audit_log WHERE project_id = $1 AND source_kind = 'ai_proposal'",
        seed.project_id,
    )
    .await;
    assert_eq!(audits, 3, "no command executed twice across the crash");
    let notifications = scalar_i64(
        &pool,
        "SELECT count(*) FROM integration.notification \
         WHERE project_id = $1 AND kind = 'change_applied'",
        seed.project_id,
    )
    .await;
    assert_eq!(notifications, 3, "one notification per proposal, no duplicates");
    let (task_revision, milestone_revision): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT revision FROM delivery.task WHERE id = $2), \
                (SELECT revision FROM delivery.milestone WHERE id = $3)",
    )
    .bind(seed.project_id)
    .bind(task_102_id())
    .bind(milestone_id())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(task_revision, 8, "single forward move despite the crash+retry");
    assert_eq!(milestone_revision, 5);

    // The job and event closed out normally.
    let job_status: String =
        sqlx::query_scalar("SELECT status FROM integration.job WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job_status, "completed");
    let event_status: String =
        sqlx::query_scalar("SELECT status FROM integration.inbound_event WHERE id = $1")
            .bind(event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(event_status, "applied");

    // Idempotent redelivery on top: a completed run is AlreadyDone, the
    // handler completes as a no-op, nothing new appears.
    let outcome = orch.handle_ai_job(&pm_infrastructure::job_queue::ClaimedJob {
        job_id,
        event_id,
        project_id: seed.project_id,
    }).await.unwrap();
    assert_eq!(outcome, HandleOutcome::Completed);
    let runs_after_redelivery: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.analysis_run WHERE event_id = $1")
            .bind(event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(runs_after_redelivery, 1);
    let audits_after_redelivery = scalar_i64(
        &pool,
        "SELECT count(*) FROM governance.audit_log WHERE project_id = $1 AND source_kind = 'ai_proposal'",
        seed.project_id,
    )
    .await;
    assert_eq!(audits_after_redelivery, 3);
}
