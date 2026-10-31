//! Task 15 acceptance: the three human powers of the V2 supervision desk —
//! veto (post-hoc rollback through real snapshot compensation), the freeze
//! brake (claim-level stop of the AI), and red-line confirmation — plus the
//! notification feed, against a real Postgres with the deterministic
//! FakeModel fixtures.
//!
//! Proposals are produced by driving the real orchestrator chain
//! (`handle_ai_job`) over the seeded fixtures: `cache_overlap` yields three
//! executed proposals (update_task_status, create_task, adjust_milestone_plan),
//! `redline_acceptance` yields one `redline_pending` proposal with its pending
//! `governance.approval_request`. The seeded rows reuse the FakeModel's
//! hard-coded ids (project `1111…1111`, task `…2222` at revision 7, milestone
//! `…3333` at revision 4) exactly like the Task 14 suite.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use pm_ai::model::AnalysisModel;
use pm_ai::orchestrator::Orchestrator;
use pm_ai::FakeModel;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::supervision::{
    DecideRequest, Decision, DecisionOutcome, SupervisionError, VetoOutcome, VetoRequest,
    decide_approval, freeze, notifications_feed, unfreeze, veto,
};
use pm_application::{CommandError, PgCommandExecutor, PolicyEngine};
use pm_infrastructure::job_queue::{self, ClaimedJob};
use serde_json::{Value, json};
use uuid::Uuid;

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
/// TASK-102 at revision 7 (`running`) and the milestone at revision 4.
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
         VALUES ($1, $2, 'FIXTURE', 'Supervision Project')",
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
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', 'Supervisor')")
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
         VALUES ($1, $2, $3, 'TASK-102 implement cache layer', 'running', 7)",
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

/// One inbound event plus its queued `ai_analysis` job; returns the
/// ClaimedJob a worker would hold.
async fn seed_job(
    pool: &sqlx::PgPool,
    seed: &Seed,
    event_type: &str,
    payload: Value,
) -> ClaimedJob {
    let job_id = Uuid::new_v4();
    let event_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash) \
         VALUES ($1, $2, $3, 'test', $4, $5, $6, 'hash')",
    )
    .bind(event_id)
    .bind(seed.project_id)
    .bind(seed.dev_agent_id)
    .bind(event_type)
    .bind(event_id.to_string())
    .bind(payload)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO integration.job (id, project_id, event_id, job_type) \
         VALUES ($1, $2, $3, 'ai_analysis')",
    )
    .bind(job_id)
    .bind(seed.project_id)
    .bind(event_id)
    .execute(pool)
    .await
    .unwrap();

    ClaimedJob {
        job_id,
        event_id,
        project_id: seed.project_id,
    }
}

/// The orchestrator under test: FakeModel behind the model port, the
/// for-tests policy, and the real Postgres executor (Task 14 wiring).
fn orchestrator(pool: &sqlx::PgPool) -> Orchestrator {
    let policy = Arc::new(PolicyEngine::for_tests());
    let executor = Arc::new(PgCommandExecutor::new(pool.clone(), policy.clone()));
    let model: Arc<dyn AnalysisModel> = Arc::new(FakeModel::from_fixtures());
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

/// Drives the real chain over the `cache_overlap` fixture: three executed
/// proposals.
async fn run_cache_overlap(pool: &sqlx::PgPool, seed: &Seed) -> ClaimedJob {
    let job = seed_job(
        pool,
        seed,
        "technical_design_submitted",
        json!({
            "fixture": "cache_overlap",
            "task_ref": task_102_id().to_string(),
            "milestone_ref": milestone_id().to_string(),
        }),
    )
    .await;
    orchestrator(pool)
        .handle_ai_job(&job)
        .await
        .expect("cache_overlap chain must complete");
    job
}

/// Drives the real chain over the `redline_acceptance` fixture: one
/// `redline_pending` proposal with its pending approval request.
async fn run_redline_acceptance(pool: &sqlx::PgPool, seed: &Seed) -> ClaimedJob {
    let job = seed_job(
        pool,
        seed,
        "milestone_check",
        json!({
            "fixture": "redline_acceptance",
            "milestone_ref": milestone_id().to_string(),
        }),
    )
    .await;
    orchestrator(pool)
        .handle_ai_job(&job)
        .await
        .expect("redline_acceptance chain must complete");
    job
}

/// (1) veto over an executed proposal rolls the command back, records the
/// intervention and the feed notification — and a repeat veto is an
/// idempotent success that only adds one more intervention row.
#[sqlx::test(migrations = "../../migrations")]
async fn veto_executed_proposal_rolls_back_then_reports_already_compensated(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    run_cache_overlap(&pool, &seed).await;

    let (proposal_id, executed_audit_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, executed_audit_id FROM ai.change_proposal \
         WHERE project_id = $1 AND command_type = 'update_task_status'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let outcome = veto(
        &pool,
        &test_executor(&pool),
        VetoRequest {
            project_id: seed.project_id,
            proposal_id,
            actor_id: seed.human_id,
            reason: "方向错误，撤销".to_string(),
        },
    )
    .await
    .unwrap();
    let VetoOutcome::RolledBack { new_audit_id } = outcome else {
        panic!("expected RolledBack, got {outcome:?}");
    };
    assert_ne!(new_audit_id, executed_audit_id);

    // The task is back to its pre-proposal status; revisions only move
    // forward (7 -> 8 executed -> 9 restore).
    let (status, revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.task WHERE id = $1")
            .bind(task_102_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "running");
    assert_eq!(revision, 9);

    // The original audit row points at its rollback row.
    let compensated: Uuid = sqlx::query_scalar(
        "SELECT compensated_command_id FROM governance.audit_log WHERE id = $1",
    )
    .bind(executed_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(compensated, new_audit_id);

    // intervention('veto') carrying both anchors.
    let interventions: Vec<(String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT intervention_type, target_proposal_id, target_command_id \
         FROM governance.human_intervention WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(interventions.len(), 1);
    assert_eq!(interventions[0].0, "veto");
    assert_eq!(interventions[0].1, proposal_id);
    assert_eq!(interventions[0].2, executed_audit_id);

    // notification('veto_applied') reached the feed with the rollback anchor.
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

    // Second veto: idempotent success semantics (HTTP 200 at the route), one
    // more intervention row, no duplicate notification.
    let second = veto(
        &pool,
        &test_executor(&pool),
        VetoRequest {
            project_id: seed.project_id,
            proposal_id,
            actor_id: seed.human_id,
            reason: "repeat veto".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(second, VetoOutcome::AlreadyCompensated);
    let interventions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.human_intervention WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(interventions, 2, "the repeat veto still records itself");
    let notifications: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.notification \
         WHERE project_id = $1 AND kind = 'veto_applied'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(notifications, 1);
}

/// (2) veto fails without overwriting newer state when the target moved on
/// since the proposal executed; the failure lands as intervention('rollback_failed').
#[sqlx::test(migrations = "../../migrations")]
async fn veto_reports_rollback_failed_when_target_moved_on(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    run_cache_overlap(&pool, &seed).await;

    let (proposal_id, executed_audit_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, executed_audit_id FROM ai.change_proposal \
         WHERE project_id = $1 AND command_type = 'update_task_status'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Simulate a later edit: the task moved on since the proposal executed.
    sqlx::query("UPDATE delivery.task SET revision = revision + 1 WHERE id = $1")
        .bind(task_102_id())
        .execute(&pool)
        .await
        .unwrap();

    let err = veto(
        &pool,
        &test_executor(&pool),
        VetoRequest {
            project_id: seed.project_id,
            proposal_id,
            actor_id: seed.human_id,
            reason: "too late, but try".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, SupervisionError::Command(CommandError::RevisionConflict { .. })),
        "expected RevisionConflict, got {err:?}"
    );

    // The newer state is untouched and the original audit row stays open.
    let (status, revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.task WHERE id = $1")
            .bind(task_102_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "awaiting_decision");
    assert_eq!(revision, 9);
    let compensated: Option<Uuid> = sqlx::query_scalar(
        "SELECT compensated_command_id FROM governance.audit_log WHERE id = $1",
    )
    .bind(executed_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(compensated.is_none(), "a failed rollback must not mark the audit row compensated");

    let failures: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.human_intervention \
         WHERE project_id = $1 AND intervention_type = 'rollback_failed'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failures, 1, "the failed rollback is on record");
}

/// (2b) vetoing an executed-but-irreversible proposal (an approved
/// `sign_acceptance` carries no compensation spec) must still put the human
/// veto on record: intervention('rollback_failed', executed_at NULL) plus the
/// feed notification — never a silent refusal.
#[sqlx::test(migrations = "../../migrations")]
async fn veto_of_irreversible_command_records_rollback_failed(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    run_redline_acceptance(&pool, &seed).await;

    // The human approves the red line; sign_acceptance executes (irreversible).
    let (approval_id, proposal_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, proposal_id FROM governance.approval_request WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
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
    assert!(matches!(outcome, DecisionOutcome::Executed { .. }));

    // The later veto cannot roll it back — but must be fully recorded.
    let err = veto(
        &pool,
        &test_executor(&pool),
        VetoRequest {
            project_id: seed.project_id,
            proposal_id,
            actor_id: seed.human_id,
            reason: "客户撤回验收".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, SupervisionError::Command(CommandError::Validation(_))),
        "expected Validation (no compensation spec), got {err:?}"
    );

    let (failures, not_executed): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE executed_at IS NULL) \
         FROM governance.human_intervention \
         WHERE project_id = $1 AND intervention_type = 'rollback_failed'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failures, 1, "the veto must be on record");
    assert_eq!(not_executed, 1, "rollback_failed is recorded as not executed");

    let notifications: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.notification \
         WHERE project_id = $1 AND kind = 'rollback_failed'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(notifications, 1, "the feed carries the refused rollback");
}

/// (3) Approve executes the red line exactly once through the human channel;
/// a repeat decision is `AlreadyDecided` and executes nothing more.
#[sqlx::test(migrations = "../../migrations")]
async fn approve_executes_once_and_second_decide_is_already_decided(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    run_redline_acceptance(&pool, &seed).await;

    let (approval_id, proposal_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, proposal_id FROM governance.approval_request WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

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

    // The red line really executed through the human path.
    let (status, revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.milestone WHERE id = $1")
            .bind(milestone_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "accepted");
    assert_eq!(revision, 5);

    // The proposal carries the executed linkage; the approval is approved.
    let (proposal_status, executed_audit_id): (String, Uuid) = sqlx::query_as(
        "SELECT status, executed_audit_id FROM ai.change_proposal WHERE id = $1",
    )
    .bind(proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(proposal_status, "executed");
    assert_eq!(executed_audit_id, receipt.audit_id);
    let approval_status: String =
        sqlx::query_scalar("SELECT status FROM governance.approval_request WHERE id = $1")
            .bind(approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(approval_status, "approved");

    // The audit row carries the approval identity — the Task 6 join anchor.
    let (source_kind, source_ref): (String, Uuid) = sqlx::query_as(
        "SELECT source_kind, source_ref FROM governance.audit_log WHERE id = $1",
    )
    .bind(receipt.audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source_kind, "redline_approval");
    assert_eq!(source_ref, approval_id);

    // A repeat decision is AlreadyDecided and must not execute again.
    let err = decide_approval(
        &pool,
        &test_executor(&pool),
        DecideRequest {
            approval_id,
            actor_id: seed.human_id,
            decision: Decision::Approve,
            reason: "repeat".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, SupervisionError::Command(CommandError::AlreadyDecided { .. })),
        "expected AlreadyDecided, got {err:?}"
    );
    let executions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND source_kind = 'redline_approval'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(executions, 1, "the command must execute exactly once");
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM delivery.milestone WHERE id = $1")
            .bind(milestone_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revision, 5, "no second execution bumps the milestone again");
}

/// (4) freeze switches the project to `controlled` so `job_queue::claim`
/// hands out nothing; unfreeze flips it back and claiming resumes.
#[sqlx::test(migrations = "../../migrations")]
async fn freeze_stops_claiming_and_unfreeze_resumes(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    let job = seed_job(&pool, &seed, "bug_report", json!({ "text": "boom" })).await;

    freeze(&pool, seed.project_id, seed.human_id, "发布冻结，先刹车").await
        .unwrap();
    let mode: String =
        sqlx::query_scalar("SELECT automation_mode FROM ai.project_ai_config WHERE project_id = $1")
            .bind(seed.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mode, "controlled");
    assert!(
        job_queue::claim(&pool, "ai_analysis", "worker-1", 60).await.is_none(),
        "a frozen project must not hand out jobs"
    );
    let interventions: Vec<String> = sqlx::query_as(
        "SELECT intervention_type FROM governance.human_intervention WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(t,)| t)
    .collect();
    assert_eq!(interventions, vec!["freeze".to_string()]);
    let kinds: Vec<String> = sqlx::query_as(
        "SELECT kind FROM integration.notification WHERE project_id = $1",
    )
    .bind(seed.project_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(k,)| k)
    .collect();
    assert_eq!(kinds, vec!["project_frozen".to_string()]);

    unfreeze(&pool, seed.project_id, seed.human_id, "恢复 AI 领航").await
        .unwrap();
    let claimed = job_queue::claim(&pool, "ai_analysis", "worker-1", 60)
        .await
        .expect("an unfrozen project hands the job out again");
    assert_eq!(claimed.project_id, seed.project_id);
    assert_eq!(claimed.event_id, job.event_id);
    let mode: String =
        sqlx::query_scalar("SELECT automation_mode FROM ai.project_ai_config WHERE project_id = $1")
            .bind(seed.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mode, "ai_lead");
    let kinds: Vec<String> = sqlx::query_as(
        "SELECT kind FROM integration.notification WHERE project_id = $1 ORDER BY created_at, kind",
    )
    .bind(seed.project_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(k,)| k)
    .collect();
    assert_eq!(
        kinds,
        vec!["project_frozen".to_string(), "project_unfrozen".to_string()]
    );
}

/// (5) a frozen project's red-line confirmation still executes: the human
/// decision path never goes through the claim layer, so the brake cannot
/// strand a pending confirmation.
#[sqlx::test(migrations = "../../migrations")]
async fn frozen_project_human_confirm_still_executes(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    run_redline_acceptance(&pool, &seed).await;

    freeze(&pool, seed.project_id, seed.human_id, "冻结期间处理确认").await
        .unwrap();

    let approval_id: Uuid =
        sqlx::query_scalar("SELECT id FROM governance.approval_request WHERE project_id = $1")
            .bind(seed.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let outcome = decide_approval(
        &pool,
        &test_executor(&pool),
        DecideRequest {
            approval_id,
            actor_id: seed.human_id,
            decision: Decision::Approve,
            reason: "冻结不影响人类确认".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(outcome, DecisionOutcome::Executed { .. }), "got {outcome:?}");

    let (status, revision): (String, i64) =
        sqlx::query_as("SELECT status, revision FROM delivery.milestone WHERE id = $1")
            .bind(milestone_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "accepted");
    assert_eq!(revision, 5, "the human confirm path executes despite the freeze");
}

/// (6) the veto window expiring does not disable the veto: 「随时否决」 — the
/// rollback still succeeds long after the window closed.
#[sqlx::test(migrations = "../../migrations")]
async fn veto_still_rolls_back_after_veto_window_expired(pool: sqlx::PgPool) {
    let seed = seed_world(&pool).await;
    run_cache_overlap(&pool, &seed).await;

    let proposal_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM ai.change_proposal \
         WHERE project_id = $1 AND command_type = 'adjust_milestone_plan'",
    )
    .bind(seed.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Force the change_applied notification's veto window into the past.
    sqlx::query(
        "UPDATE integration.notification \
         SET veto_window_until = now() - interval '1 hour' WHERE source_proposal_id = $1",
    )
    .bind(proposal_id)
    .execute(&pool)
    .await
    .unwrap();
    let window: DateTime<Utc> = sqlx::query_scalar(
        "SELECT veto_window_until FROM integration.notification WHERE source_proposal_id = $1",
    )
    .bind(proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(window < Utc::now(), "premise: the window must already be expired");

    let outcome = veto(
        &pool,
        &test_executor(&pool),
        VetoRequest {
            project_id: seed.project_id,
            proposal_id,
            actor_id: seed.human_id,
            reason: "窗口过了也能否决".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(outcome, VetoOutcome::RolledBack { .. }), "got {outcome:?}");

    // The milestone move is undone: back to the seeded plan, revision forward.
    let (planned_at, revision): (Option<DateTime<Utc>>, i64) = sqlx::query_as(
        "SELECT planned_at, revision FROM delivery.milestone WHERE id = $1",
    )
    .bind(milestone_id())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(planned_at, Some(seeded_planned_at()));
    assert_eq!(revision, 6, "4 seeded -> 5 executed -> 6 restored");
    let moved: Option<DateTime<Utc>> = Some(moved_planned_at());
    assert_ne!(planned_at, moved);
}

/// (7) the notification feed reads newest-first with `since` as an exclusive
/// cursor and a working limit.
#[sqlx::test(migrations = "../../migrations")]
async fn notifications_feed_orders_desc_and_paginates(pool: sqlx::PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "SUP-FEED".to_string(),
            project_name: "supervision feed".to_string(),
        },
    )
    .await
    .unwrap();

    // Five notifications at controlled, strictly distinct timestamps:
    // idx 0 is oldest (4 minutes ago), idx 4 is newest (now).
    let kinds = ["a_oldest", "b_middle", "c_middle", "d_middle", "e_newest"];
    for (idx, kind) in kinds.iter().enumerate() {
        sqlx::query(
            "INSERT INTO integration.notification (project_id, kind, payload, created_at) \
             VALUES ($1, $2, $3, now() - make_interval(mins => $4))",
        )
        .bind(boot.project_id)
        .bind(kind)
        .bind(json!({ "seq": idx }))
        .bind((4 - idx) as i32)
        .execute(&pool)
        .await
        .unwrap();
    }

    let feed = notifications_feed(&pool, boot.project_id, None, 100).await.unwrap();
    assert_eq!(feed.len(), 5);
    for pair in feed.windows(2) {
        assert!(
            pair[0].created_at > pair[1].created_at,
            "feed must be strictly newest-first"
        );
    }
    assert_eq!(feed[0].kind, "e_newest");
    assert_eq!(feed[0].payload["seq"], json!(4));
    assert_eq!(feed[4].kind, "a_oldest");

    // The limit truncates from the top (the newest rows survive).
    let top2 = notifications_feed(&pool, boot.project_id, None, 2).await.unwrap();
    assert_eq!(top2.len(), 2);
    assert_eq!(top2[0].id, feed[0].id);
    assert_eq!(top2[1].id, feed[1].id);

    // `since` is an exclusive cursor: everything strictly newer than the
    // second row is exactly the newest row.
    let newer = notifications_feed(&pool, boot.project_id, Some(feed[1].created_at), 100)
        .await
        .unwrap();
    assert_eq!(newer.len(), 1);
    assert_eq!(newer[0].id, feed[0].id);
}
