//! Task 12 acceptance: the analysis-run lifecycle (fresh insert from
//! config, crash-recovery reuse, AlreadyDone, retry rows inheriting
//! attempt_count) and the context builder (ref loading, cross-project
//! isolation, snapshot round-trip) against a real Postgres. The semantic
//! rulings under test are documented on `pm_ai::context` itself.

use pm_ai::context::{build, fail_run, finish_run, save_snapshot, start_run, ContextError};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use uuid::Uuid;

const MODEL_NAME: &str = "ctx-fake-model";
/// Non-default value so copies into ai.analysis_run are observable, not
/// column defaults.
const POLICY_VERSION: i32 = 3;

struct Seed {
    project_id: Uuid,
    principal_id: Uuid,
}

/// The model_config seeded for every project here; non-default content
/// for the same reason as POLICY_VERSION.
fn model_config() -> Value {
    json!({"temperature": 0.2})
}

/// Minimal bootstrap-equivalent rows: customer, project, AI principal and
/// an ai_lead-enabled config. Bare SQL on purpose. `code` is derived from
/// the project id so one test can seed two projects.
async fn seed_project(pool: &sqlx::PgPool) -> Seed {
    let customer_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let principal_id = Uuid::new_v4();

    sqlx::query("INSERT INTO core.customer (id, name, tenant_id) VALUES ($1, 'Acme', $2)")
        .bind(customer_id)
        .bind(SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $3, 'Context Project', $4)",
    )
    .bind(project_id)
    .bind(customer_id)
    .bind(project_id.to_string())
    .bind(SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'AI PM')")
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ai.project_ai_config \
         (project_id, ai_principal_id, model_provider, model_name, model_config, \
          policy_version, automation_mode, enabled, tenant_id) \
         VALUES ($1, $2, 'test', $3, $4, $5, 'ai_lead', true, \
                  (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(principal_id)
    .bind(MODEL_NAME)
    .bind(model_config())
    .bind(POLICY_VERSION)
    .execute(pool)
    .await
    .unwrap();

    Seed {
        project_id,
        principal_id,
    }
}

/// One inbound event carrying `payload`; returns its id.
async fn seed_event(pool: &sqlx::PgPool, seed: &Seed, event_type: &str, payload: Value) -> Uuid {
    let event_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, tenant_id) \
         VALUES ($1, $2, $3, 'test', $4, $5, $6, 'hash', \
                  (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(event_id)
    .bind(seed.project_id)
    .bind(seed.principal_id)
    .bind(event_type)
    .bind(event_id.to_string())
    .bind(payload)
    .execute(pool)
    .await
    .unwrap();
    event_id
}

/// One bare task row (no milestone) in the given project.
async fn seed_task(pool: &sqlx::PgPool, project_id: Uuid) -> Uuid {
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, title, tenant_id) \
         VALUES ($1, $2, 'Foreign task', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
        .bind(task_id)
        .bind(project_id)
        .execute(pool)
        .await
        .unwrap();
    task_id
}

/// Seed one row in each of the three ref targets (task, milestone,
/// requirement revision, the last via its parent requirement); returns
/// (task_id, milestone_id, revision_id).
async fn seed_delivery_rows(pool: &sqlx::PgPool, seed: &Seed) -> (Uuid, Uuid, Uuid) {
    let milestone_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    let requirement_id = Uuid::new_v4();
    let revision_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO delivery.milestone (id, project_id, name, status, revision, tenant_id) \
         VALUES ($1, $2, 'Design frozen', 'planned', 4, \
                  (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(milestone_id)
    .bind(seed.project_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO delivery.task \
         (id, project_id, milestone_id, title, description, status, priority, revision, tenant_id) \
         VALUES ($1, $2, $3, 'Write API spec', 'OpenAPI 3.1 first pass', 'running', 'high', 7, \
                  (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(seed.project_id)
    .bind(milestone_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO delivery.requirement (id, project_id, code, title, status, tenant_id) \
         VALUES ($1, $2, $3, 'Cache overlap handling', 'approved', \
                  (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(requirement_id)
    .bind(seed.project_id)
    .bind(requirement_id.to_string())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO delivery.requirement_revision \
         (id, requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, $3, 2, $4, 'approved', 'hash-1', $5, \
                  (SELECT tenant_id FROM core.project WHERE id = $3))",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .bind(seed.project_id)
    .bind(json!({"goal": "no stale reads"}))
    .bind(seed.principal_id)
    .execute(pool)
    .await
    .unwrap();

    (task_id, milestone_id, revision_id)
}

/// How many runs an event has (count(*) is INT8).
async fn run_count(pool: &sqlx::PgPool, event_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM ai.analysis_run WHERE event_id = $1")
        .bind(event_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn start_run_creates_analyzing_row_from_config(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let event_id = seed_event(&pool, &seed, "technical_design_submitted", json!({})).await;

    let run_id = start_run(&pool, event_id).await.expect("start_run");

    let (status, project_id, model_name, policy_version, config_snapshot, attempt_count, started, finished) =
        sqlx::query_as::<_, (String, Uuid, String, i32, Value, i32, bool, bool)>(
            "SELECT status, project_id, model_name, policy_version, config_snapshot, \
                    attempt_count, started_at IS NOT NULL, finished_at IS NOT NULL \
             FROM ai.analysis_run WHERE id = $1",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "analyzing");
    assert_eq!(project_id, seed.project_id, "project_id comes from the event");
    assert_eq!(model_name, MODEL_NAME);
    assert_eq!(policy_version, POLICY_VERSION);
    assert_eq!(config_snapshot, model_config(), "config_snapshot copies model_config");
    assert_eq!(attempt_count, 0, "first run starts at attempt_count 0");
    assert!(started, "started_at is set on insert");
    assert!(!finished);
}

#[sqlx::test(migrations = "../../migrations")]
async fn start_run_reuses_open_run_for_crash_recovery(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let event_id = seed_event(&pool, &seed, "task.updated", json!({})).await;

    let first = start_run(&pool, event_id).await.expect("first start_run");
    let second = start_run(&pool, event_id).await.expect("second start_run");
    assert_eq!(first, second, "an open run must be adopted, not duplicated");

    assert_eq!(run_count(&pool, event_id).await, 1);

    let status: String = sqlx::query_scalar("SELECT status FROM ai.analysis_run WHERE id = $1")
        .bind(first)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "analyzing");
}

#[sqlx::test(migrations = "../../migrations")]
async fn completed_run_blocks_restart_with_already_done(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let event_id = seed_event(&pool, &seed, "task.updated", json!({})).await;
    let run_id = start_run(&pool, event_id).await.expect("start_run");

    let result = json!({"findings": [], "proposals": []});
    let self_review = json!({"gate": "auto"});
    finish_run(&pool, run_id, &result, &self_review)
        .await
        .expect("finish_run");

    let (status, stored_result, stored_review, finished): (String, Value, Value, bool) =
        sqlx::query_as(
            "SELECT status, result, self_review, finished_at IS NOT NULL \
             FROM ai.analysis_run WHERE id = $1",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(stored_result, result);
    assert_eq!(stored_review, self_review);
    assert!(finished);

    let err = start_run(&pool, event_id)
        .await
        .expect_err("a completed event must refuse a restart");
    assert!(matches!(err, ContextError::AlreadyDone(id) if id == event_id));
    assert_eq!(run_count(&pool, event_id).await, 1, "AlreadyDone inserts nothing");
}

#[sqlx::test(migrations = "../../migrations")]
async fn failed_run_retries_as_new_row_inheriting_attempt_count(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let event_id = seed_event(&pool, &seed, "task.updated", json!({})).await;
    let first = start_run(&pool, event_id).await.expect("first start_run");

    fail_run(&pool, first, "model_timeout")
        .await
        .expect("fail_run");
    let (status, failure_code, finished): (String, Option<String>, bool) = sqlx::query_as(
        "SELECT status, failure_code, finished_at IS NOT NULL \
         FROM ai.analysis_run WHERE id = $1",
    )
    .bind(first)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(failure_code.as_deref(), Some("model_timeout"));
    assert!(finished, "failure is also a terminal state");

    let second = start_run(&pool, event_id)
        .await
        .expect("retry after a failed run must be allowed");
    assert_ne!(first, second, "retry means a fresh row");

    let (status, attempt_count): (String, i32) =
        sqlx::query_as("SELECT status, attempt_count FROM ai.analysis_run WHERE id = $1")
            .bind(second)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "analyzing");
    assert_eq!(attempt_count, 1, "new run inherits old attempt_count + 1");
    assert_eq!(run_count(&pool, event_id).await, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn build_loads_scoped_refs_and_config(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let (task_id, milestone_id, revision_id) = seed_delivery_rows(&pool, &seed).await;
    let payload = json!({
        "task_ref": task_id.to_string(),
        "milestone_ref": milestone_id.to_string(),
        "requirement_ref": revision_id.to_string(),
    });
    let event_id = seed_event(&pool, &seed, "technical_design_submitted", payload.clone()).await;

    let ctx = build(&pool, event_id).await.expect("build");

    assert_eq!(ctx.project_id, seed.project_id);
    assert_eq!(ctx.event_id, event_id);
    assert_eq!(ctx.event_type, "technical_design_submitted");
    assert_eq!(ctx.payload, payload);
    assert_eq!(ctx.config, model_config(), "config copies model_config");
    assert_eq!(ctx.policy_version, POLICY_VERSION);

    let task = ctx.task.expect("task_ref must resolve");
    assert_eq!(task["id"], task_id.to_string());
    assert_eq!(task["title"], "Write API spec");
    assert_eq!(task["revision"], json!(7));

    let milestone = ctx.milestone.expect("milestone_ref must resolve");
    assert_eq!(milestone["id"], milestone_id.to_string());
    assert_eq!(milestone["name"], "Design frozen");

    let revision = ctx
        .requirement_revision
        .expect("requirement_ref must resolve");
    assert_eq!(revision["id"], revision_id.to_string());
    assert_eq!(revision["version"], json!(2));
    assert_eq!(revision["state"], "approved");
}

#[sqlx::test(migrations = "../../migrations")]
async fn missing_ref_is_not_found_and_fails_the_run(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let ghost = Uuid::new_v4();
    let event_id = seed_event(
        &pool,
        &seed,
        "task.updated",
        json!({"task_ref": ghost.to_string()}),
    )
    .await;
    let run_id = start_run(&pool, event_id).await.expect("start_run");

    let err = build(&pool, event_id)
        .await
        .expect_err("a task_ref to nothing must fail");
    match err {
        ContextError::NotFound { r#ref } => assert_eq!(r#ref, ghost.to_string()),
        other => panic!("expected NotFound, got {other:?}"),
    }

    fail_run(&pool, run_id, "context_not_found")
        .await
        .expect("fail_run");
    let (status, failure_code, finished): (String, Option<String>, bool) = sqlx::query_as(
        "SELECT status, failure_code, finished_at IS NOT NULL \
         FROM ai.analysis_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(failure_code.as_deref(), Some("context_not_found"));
    assert!(finished);
}

#[sqlx::test(migrations = "../../migrations")]
async fn foreign_project_ref_is_not_found(pool: sqlx::PgPool) {
    let home = seed_project(&pool).await;
    let foreign = seed_project(&pool).await;
    let foreign_task = seed_task(&pool, foreign.project_id).await;
    let event_id = seed_event(
        &pool,
        &home,
        "task.updated",
        json!({"task_ref": foreign_task.to_string()}),
    )
    .await;

    // The row exists in delivery.task but in another project: the
    // project_id scope must make it indistinguishable from a missing ref.
    let err = build(&pool, event_id)
        .await
        .expect_err("a cross-project ref must fail");
    match err {
        ContextError::NotFound { r#ref } => assert_eq!(r#ref, foreign_task.to_string()),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn saved_snapshot_round_trips_core_ids(pool: sqlx::PgPool) {
    let seed = seed_project(&pool).await;
    let event_id = seed_event(&pool, &seed, "task.updated", json!({"note": "no refs"})).await;
    let run_id = start_run(&pool, event_id).await.expect("start_run");

    let ctx = build(&pool, event_id).await.expect("build");
    assert_eq!(ctx.task, None, "absent refs stay None");

    save_snapshot(&pool, run_id, &ctx)
        .await
        .expect("save_snapshot");

    let snapshot: Value =
        sqlx::query_scalar("SELECT context_snapshot FROM ai.analysis_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(snapshot["event_id"], event_id.to_string());
    assert_eq!(snapshot["project_id"], seed.project_id.to_string());
    assert_eq!(snapshot["policy_version"], json!(POLICY_VERSION));
    assert_eq!(snapshot["event_type"], "task.updated");
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_event_is_event_not_found_everywhere(pool: sqlx::PgPool) {
    let missing = Uuid::new_v4();

    let err = start_run(&pool, missing)
        .await
        .expect_err("start_run on an unknown event");
    assert!(matches!(err, ContextError::EventNotFound(id) if id == missing));

    let err = build(&pool, missing)
        .await
        .expect_err("build on an unknown event");
    assert!(matches!(err, ContextError::EventNotFound(id) if id == missing));
}
