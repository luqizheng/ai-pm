//! PA08-03 Run transition guard integration tests against real PostgreSQL.

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_context::{get_run_context, hash_lease_token, LeaseError};
use pm_application::run_dispatch::{claim_task_run, RunDispatchConfig, NoopExecutor};
use pm_application::run_transition_guard::{
    lock_and_validate_transition, RunKind, RunStatus, RunTransitionError,
    RunTransitionGuardError,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    delivery_run_id: Uuid,
    review_run_id: Uuid,
    human_id: Uuid,
}

async fn seed(pool: &PgPool, code: &str) -> Fx {
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
    .bind(format!("pa08-{code}-agent"))
    .fetch_one(pool)
    .await
    .unwrap();

    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, 'PA08 transition guard', 'ready', 1, $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let repository_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'pa08-guard-repo', $2, 'verified', 'base-sha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(format!("git@host:{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository \
         (task_id, project_id, repository_id, write_access, base_ref, path_scope, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', ARRAY['**'], \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(boot.project_id)
    .bind(repository_id)
    .execute(pool)
    .await
    .unwrap();

    let delivery_run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, run_kind, tenant_id) \
         VALUES ($1, $2, 1, 'queued', 'delivery', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let review_run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, run_kind, role_id, executor_kind, \
          review_policy_version, execution_mode, tenant_id) \
         VALUES ($1, $2, 1, 'queued', 'review', 'reviewer', 'platform', 'v1', 'spawn', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();

    Fx {
        project_id: boot.project_id,
        task_id,
        delivery_run_id,
        review_run_id,
        human_id: boot.human_id,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn delivery_guard_locks_and_allows_only_the_expected_edge(pool: PgPool) {
    let fx = seed(&pool, "PA08-RG-DELIVERY").await;

    let mut tx = pool.begin().await.unwrap();
    let from = lock_and_validate_transition(
        &mut tx,
        fx.project_id,
        fx.delivery_run_id,
        RunKind::Delivery,
        RunStatus::Claimed,
    )
    .await
    .unwrap();
    assert_eq!(from, RunStatus::Queued);

    let updated = sqlx::query(
        "UPDATE delivery.task_run SET status = 'claimed' \
          WHERE id = $1 AND project_id = $2 AND status = $3",
    )
    .bind(fx.delivery_run_id)
    .bind(fx.project_id)
    .bind(from.as_str())
    .execute(&mut *tx)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(updated, 1);
    tx.commit().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let from = lock_and_validate_transition(
        &mut tx,
        fx.project_id,
        fx.delivery_run_id,
        RunKind::Delivery,
        RunStatus::Running,
    )
    .await
    .unwrap();
    assert_eq!(from, RunStatus::Claimed);
    tx.rollback().await.unwrap();

    let current: String = sqlx::query_scalar(
        "SELECT status FROM delivery.task_run WHERE id = $1",
    )
    .bind(fx.delivery_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(current, "claimed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn reviewer_run_uses_its_own_transition_matrix(pool: PgPool) {
    let fx = seed(&pool, "PA08-RG-REVIEW").await;

    let mut tx = pool.begin().await.unwrap();
    let from = lock_and_validate_transition(
        &mut tx,
        fx.project_id,
        fx.review_run_id,
        RunKind::Review,
        RunStatus::Running,
    )
    .await
    .unwrap();
    assert_eq!(from, RunStatus::Queued);
    tx.rollback().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let wrong_kind = lock_and_validate_transition(
        &mut tx,
        fx.project_id,
        fx.review_run_id,
        RunKind::Delivery,
        RunStatus::Claimed,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        wrong_kind,
        RunTransitionGuardError::Invalid(RunTransitionError::IllegalEdge {
            kind: RunKind::Review,
            ..
        })
    ));
    tx.rollback().await.unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn terminal_run_is_immutable(pool: PgPool) {
    let fx = seed(&pool, "PA08-RG-TERMINAL").await;
    sqlx::query(
        "UPDATE delivery.task_run SET status = 'succeeded', outcome = 'succeeded' WHERE id = $1",
    )
    .bind(fx.delivery_run_id)
    .execute(&pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let error = lock_and_validate_transition(
        &mut tx,
        fx.project_id,
        fx.delivery_run_id,
        RunKind::Delivery,
        RunStatus::Running,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        RunTransitionGuardError::Invalid(RunTransitionError::TerminalImmutable("succeeded"))
    ));
    tx.rollback().await.unwrap();
}

async fn terminal_writer(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    target: RunStatus,
) -> Result<(), RunTransitionGuardError> {
    let mut tx = pool.begin().await.map_err(RunTransitionGuardError::Db)?;
    let from = lock_and_validate_transition(
        &mut tx,
        project_id,
        run_id,
        RunKind::Delivery,
        target,
    )
    .await?;

    // Hold the row lock briefly so the competing terminal writer must wait and
    // then observe the committed terminal status rather than overwrite it.
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    sqlx::query(
        "UPDATE delivery.task_run SET status = $3, outcome = $3, finished_at = now() \
          WHERE id = $1 AND project_id = $2 AND status = $4",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(target.as_str())
    .bind(from.as_str())
    .execute(&mut *tx)
    .await
    .map_err(RunTransitionGuardError::Db)?;
    tx.commit().await.map_err(RunTransitionGuardError::Db)?;
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_terminal_writers_cannot_overwrite_each_other(pool: PgPool) {
    let fx = seed(&pool, "PA08-RG-RACE").await;
    sqlx::query("UPDATE delivery.task_run SET status = 'running' WHERE id = $1")
        .bind(fx.delivery_run_id)
        .execute(&pool)
        .await
        .unwrap();

    let (succeeded, failed) = tokio::join!(
        terminal_writer(
            &pool,
            fx.project_id,
            fx.delivery_run_id,
            RunStatus::Succeeded,
        ),
        terminal_writer(
            &pool,
            fx.project_id,
            fx.delivery_run_id,
            RunStatus::Failed,
        )
    );

    assert_ne!(succeeded.is_ok(), failed.is_ok(), "exactly one terminal writer wins");
    let losing = if succeeded.is_err() { succeeded.unwrap_err() } else { failed.unwrap_err() };
    assert!(matches!(
        losing,
        RunTransitionGuardError::Invalid(RunTransitionError::TerminalImmutable(_))
    ));

    let final_status: String = sqlx::query_scalar(
        "SELECT status FROM delivery.task_run WHERE id = $1",
    )
    .bind(fx.delivery_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(final_status == "succeeded" || final_status == "failed");
}


#[sqlx::test(migrations = "../../migrations")]
async fn project_archive_cancels_active_reviewer_run(pool: PgPool) {
    let fx = seed(&pool, "PA08-RG-ARCHIVE").await;
    sqlx::query("UPDATE delivery.task_run SET status = 'running' WHERE id = $1")
        .bind(fx.review_run_id)
        .execute(&pool)
        .await
        .unwrap();

    pm_application::archive::close_project(
        &pool,
        &pm_application::run_dispatch::NoopExecutor,
        &pm_application::actor::AuditActor::principal(fx.human_id),
        SYSTEM_TENANT,
        fx.project_id,
        "pa08-review-run-archive",
    )
    .await
    .unwrap();

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delivery.task_run WHERE id = $1",
    )
    .bind(fx.review_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "cancelled");
}


#[sqlx::test(migrations = "../../migrations")]
async fn stale_lease_token_and_generation_remain_fenced(pool: PgPool) {
    let fx = seed(&pool, "PA08-RG-FENCING").await;
    let token = "pa08-current-lease-token";
    sqlx::query(
        "UPDATE delivery.task_run \
            SET status = 'claimed', lease_token_hash = $2, lease_generation = 8, \
                lease_expires_at = now() + interval '5 minutes', lease_owner = 'client:pa08' \
          WHERE id = $1",
    )
    .bind(fx.delivery_run_id)
    .bind(hash_lease_token(token))
    .execute(&pool)
    .await
    .unwrap();

    let old_token = get_run_context(
        &pool, fx.project_id, fx.delivery_run_id, "stale-token", 8,
    )
    .await
    .unwrap_err();
    assert!(matches!(old_token, LeaseError::Forbidden));

    let old_generation = get_run_context(
        &pool, fx.project_id, fx.delivery_run_id, token, 7,
    )
    .await
    .unwrap_err();
    assert!(matches!(old_generation, LeaseError::Forbidden));
}

#[sqlx::test(migrations = "../../migrations")]
async fn claim_and_project_archive_race_leaves_no_active_run(pool: PgPool) {
    let fx = seed(&pool, "PA08-RG-CLAIM-ARCHIVE").await;
    let config = RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
        timeout_secs: 1800,
        deadline_default_secs: 3600,
        agent_heartbeat_stale_secs: 900,
        progress_stale_secs: 3600,
        exclude_projects: vec![],
    };

    let claim = claim_task_run(
        &pool, fx.project_id, fx.task_id, "pa08-race-runner", None, &config,
    );
    let archive = pm_application::archive::close_project(
        &pool,
        &NoopExecutor,
        &pm_application::actor::AuditActor::principal(fx.human_id),
        SYSTEM_TENANT,
        fx.project_id,
        "pa08-claim-archive-race",
    );
    let (claim_result, archive_result) = tokio::join!(claim, archive);
    archive_result.expect("Project archive must serialize with claim");
    if let Err(error) = claim_result {
        assert!(
            matches!(error, pm_application::run_dispatch::RunDispatchError::NoRunnableRun
                | pm_application::run_dispatch::RunDispatchError::TaskNotReady(_)),
            "claim may lose to archive but must fail closed: {error:?}"
        );
    }

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delivery.task_run WHERE id = $1",
    )
    .bind(fx.delivery_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "cancelled", "archive must leave no active Run");
}
