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

    let (first, second) = tokio::join!(
        run_cleanup::claim_batch(&pool),
        run_cleanup::claim_batch(&pool),
    );
    let mut claims = first.unwrap();
    claims.extend(second.unwrap());
    assert_eq!(claims.len(), 1, "concurrent workers must claim a row exactly once");
    let first_claim = claims.pop().unwrap();
    let first_token = first_claim.claim_token.expect("claimed row has a fencing token");

    // A cleaning row must be protected by an unexpired claim lease.
    let second_claim = run_cleanup::claim_batch(&pool).await.unwrap();
    assert!(
        second_claim.is_empty(),
        "a later worker must not reclaim an unexpired cleaning row"
    );

    // Simulate a crashed worker by expiring the lease. A new claim gets a new
    // token, and the old worker must not be able to complete the newer claim.
    sqlx::query(
        "UPDATE delivery.task_run_cleanup SET next_retry_at = now() - interval '1 second' WHERE run_id = $1",
    )
    .bind(_run_id)
    .execute(&pool)
    .await
    .unwrap();

    let reclaimed = run_cleanup::claim_batch(&pool).await.unwrap();
    assert_eq!(reclaimed.len(), 1, "expired cleanup lease is recoverable");
    let new_token = reclaimed[0].claim_token.expect("reclaimed row has a new token");
    assert_ne!(first_token, new_token, "reclaim must rotate the fencing token");

    assert!(
        !run_cleanup::mark_completed(&pool, _run_id, first_token).await.unwrap(),
        "stale worker cannot complete a row reclaimed by another worker"
    );
    assert!(
        run_cleanup::mark_completed(&pool, _run_id, new_token).await.unwrap(),
        "current worker can complete its own claim"
    );

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delivery.task_run_cleanup WHERE run_id = $1",
    )
    .bind(_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
}
