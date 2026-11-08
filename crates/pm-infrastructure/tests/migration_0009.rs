//! Slice B Task 1: `0009_task_state_machine.sql` — new tables, new task
//! columns, default value and the 13-state CHECK.
//!
//! Every test runs against a freshly migrated database, so no legacy rows
//! exist: the new system has no historical task data to carry over.

use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("customer {code}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(customer_id)
    .bind(code)
    .bind(format!("project {code}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_task(pool: &PgPool, project_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, tenant_id) \
         VALUES ($1, 'task', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_principal(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', $1) RETURNING id",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_row_carries_run_budget_columns(pool: PgPool) {
    let project_id = seed_project(&pool, "M9-1").await;
    let task_id = seed_task(&pool, project_id).await;

    let row: (i32, i32, Option<String>, bool) = sqlx::query_as(
        "SELECT attempt_count, consecutive_failure_count, last_failure_category, retry_exhausted \
         FROM delivery.task WHERE project_id = $1 AND id = $2",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (0, 0, None, false));

    sqlx::query("SELECT superseded_by_task_id FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .execute(&pool)
        .await
        .expect("superseded_by_task_id column must exist");
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_run_attempt_numbers_are_unique_per_task(pool: PgPool) {
    let project_id = seed_project(&pool, "M9-2").await;
    let task_id = seed_task(&pool, project_id).await;

    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, tenant_id) \
         VALUES ($1, $2, 1, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await
    .unwrap();
    let dup = sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, tenant_id) \
         VALUES ($1, $2, 1, (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await;
    assert!(
        dup.is_err(),
        "second attempt_no = 1 must violate UNIQUE (task_id, attempt_no)"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_supersession_rejects_self_and_duplicate_edges(pool: PgPool) {
    let project_id = seed_project(&pool, "M9-3").await;
    let old_task = seed_task(&pool, project_id).await;
    let new_task = seed_task(&pool, project_id).await;
    let actor = seed_principal(&pool, "M9-3 owner").await;

    let self_edge = sqlx::query(
        "INSERT INTO delivery.task_supersession (project_id, old_task_id, new_task_id, reason, created_by, tenant_id) \
         VALUES ($1, $2, $2, 'self', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(old_task)
    .bind(actor)
    .execute(&pool)
    .await;
    assert!(
        self_edge.is_err(),
        "old_task_id = new_task_id must be rejected"
    );

    let insert = "INSERT INTO delivery.task_supersession \
                  (project_id, old_task_id, new_task_id, reason, created_by, tenant_id) \
                  VALUES ($1, $2, $3, 'replaced', $4, \
                          (SELECT tenant_id FROM core.project WHERE id = $1))";
    sqlx::query(insert)
        .bind(project_id)
        .bind(old_task)
        .bind(new_task)
        .bind(actor)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query(insert)
            .bind(project_id)
            .bind(old_task)
            .bind(new_task)
            .bind(actor)
            .execute(&pool)
            .await
            .is_err(),
        "a duplicate (old, new) edge must be rejected"
    );

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_supersession WHERE old_task_id = $1",
    )
    .bind(old_task)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_waiting_allows_one_active_row_per_task(pool: PgPool) {
    let project_id = seed_project(&pool, "M9-4").await;
    let task_id = seed_task(&pool, project_id).await;

    let insert =
        "INSERT INTO delivery.task_waiting (project_id, task_id, reason, questions, tenant_id) \
                  VALUES ($1, $2, 'needs_info', '[\"which cache?\"]', \
                          (SELECT tenant_id FROM core.project WHERE id = $1))";
    sqlx::query(insert)
        .bind(project_id)
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query(insert)
            .bind(project_id)
            .bind(task_id)
            .execute(&pool)
            .await
            .is_err(),
        "a task may have only one active waiting row"
    );

    sqlx::query(
        "UPDATE delivery.task_waiting SET ended_at = now() WHERE task_id = $1 AND ended_at IS NULL",
    )
    .bind(task_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        sqlx::query(insert)
            .bind(project_id)
            .bind(task_id)
            .execute(&pool)
            .await
            .is_ok(),
        "after closing the active row a new one must be allowed (history is kept)"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_run_failure_is_append_only(pool: PgPool) {
    let project_id = seed_project(&pool, "M9-5").await;
    let task_id = seed_task(&pool, project_id).await;

    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'failed', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run_failure (run_id, project_id, category, retryable, reason_code, tenant_id) \
         VALUES ($1, $2, 'execution', true, 'exit_code_1', \
                 (SELECT tenant_id FROM delivery.task_run WHERE id = $1))",
    )
    .bind(run_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();

    let update =
        sqlx::query("UPDATE delivery.task_run_failure SET retryable = false WHERE run_id = $1")
            .bind(run_id)
            .execute(&pool)
            .await;
    assert!(update.is_err(), "failure records must be immutable");

    let delete = sqlx::query("DELETE FROM delivery.task_run_failure WHERE run_id = $1")
        .bind(run_id)
        .execute(&pool)
        .await;
    assert!(delete.is_err(), "failure records must not be deletable");
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_defaults_to_pending_and_rejects_duplicate_idempotency_key(pool: PgPool) {
    let project_id = seed_project(&pool, "M9-6").await;

    let insert = "INSERT INTO integration.outbox_event \
                  (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key, tenant_id) \
                  VALUES ($1, 'TaskReady', 'Task', 'agg-1', '{}', 'k-1', \
                          (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id";
    let id: Uuid = sqlx::query_scalar(insert)
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let row: (String, i32, i32, i32) = sqlx::query_as(
        "SELECT status, attempts, max_attempts, schema_version \
         FROM integration.outbox_event WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, ("pending".to_string(), 0, 5, 1));

    assert!(
        sqlx::query(insert)
            .bind(project_id)
            .execute(&pool)
            .await
            .is_err(),
        "idempotency_key must be globally unique"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn constraint_accepts_all_thirteen_values_and_rejects_legacy_ones(pool: PgPool) {
    let project_id = seed_project(&pool, "M9-7").await;

    for status in [
        "draft",
        "ready",
        "claimed",
        "running",
        "awaiting_decision",
        "blocked",
        "submitted",
        "reviewing",
        "completed",
        "changes_requested",
        "failed",
        "cancelled",
        "superseded",
    ] {
        sqlx::query(
            "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
             VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(format!("ok {status}"))
        .bind(status)
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("{status} must be accepted: {e}"));
    }

    for legacy in ["backlog", "in_progress", "in_review", "done", "needs_info"] {
        let err = sqlx::query(
            "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
             VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(format!("legacy {legacy}"))
        .bind(legacy)
        .execute(&pool)
        .await;
        assert!(
            err.is_err(),
            "{legacy} must be rejected by the 13-state CHECK"
        );
    }

    let default_status: String = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, tenant_id) \
         VALUES ($1, 'default', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING status",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(default_status, "draft");
}

#[sqlx::test(migrations = "../../migrations")]
async fn failure_category_constraint_accepts_seven_values(pool: PgPool) {
    let project_id = seed_project(&pool, "M9-8").await;
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, 'cat', 'failed', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    for category in [
        "infrastructure",
        "execution",
        "validation",
        "permission",
        "policy_violation",
        "missing_dependency",
        "unknown",
    ] {
        sqlx::query("UPDATE delivery.task SET last_failure_category = $2 WHERE id = $1")
            .bind(task_id)
            .bind(category)
            .execute(&pool)
            .await
            .unwrap();
    }

    assert!(
        sqlx::query("UPDATE delivery.task SET last_failure_category = 'bogus' WHERE id = $1")
            .bind(task_id)
            .execute(&pool)
            .await
            .is_err(),
        "unknown failure categories must be rejected"
    );
}
