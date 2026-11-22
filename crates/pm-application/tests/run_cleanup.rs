//! PA08-04 real-PostgreSQL regression tests for the process-cleanup claim ledger.

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_cleanup;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn enqueue_cleanup(pool: &PgPool, code: &str) -> Uuid {
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
    let agent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', $1) RETURNING id",
    )
    .bind(format!("{code}-agent"))
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, 'cleanup ledger test', 'ready', 1, $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, run_kind, tenant_id) \
         VALUES ($1, $2, 1, 'cancelled', 'delivery', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    run_cleanup::enqueue(&mut tx, run_id, "pa08-cleanup-session", Some("codex"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    run_id
}

#[sqlx::test(migrations = "../../migrations")]
async fn cleaning_row_is_not_immediately_claimed_by_a_second_worker(pool: PgPool) {
    let _run_id = enqueue_cleanup(&pool, "PA08-CLEANUP-CLAIM").await;

    let first = run_cleanup::claim_batch(&pool).await.unwrap();
    assert_eq!(first.len(), 1, "first worker should claim the pending cleanup");

    // A cleaning row must be protected by an unexpired claim lease. The current
    // implementation leaves next_retry_at unchanged, so this assertion exposes
    // duplicate claims before the PA08-04 fix.
    let second = run_cleanup::claim_batch(&pool).await.unwrap();
    assert!(
        second.is_empty(),
        "second worker must not reclaim an unexpired cleaning row"
    );
}
