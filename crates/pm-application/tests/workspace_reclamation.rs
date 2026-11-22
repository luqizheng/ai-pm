//! PA08-04 schema-level tests for the independent workspace reclamation ledger.

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    tenant_id: Uuid,
    project_id: Uuid,
    run_id: Uuid,
    repository_id: Uuid,
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
    .bind(format!("{code}-agent"))
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, 'workspace reclamation fixture', 'ready', 1, $2, \
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
         VALUES ($1, 'reclamation-repo', $2, 'verified', 'base-sha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(boot.project_id)
    .bind(format!("git@host:{code}"))
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
    sqlx::query(
        "INSERT INTO delivery.task_run_repository \
         (run_id, project_id, repository_id, base_ref, base_commit, write_access, workspace_path, tenant_id) \
         VALUES ($1, $2, $3, 'main', 'base-sha', true, $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(run_id)
    .bind(boot.project_id)
    .bind(repository_id)
    .bind(format!("repos/repo-{repository_id}"))
    .execute(pool)
    .await
    .unwrap();
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(boot.project_id)
        .fetch_one(pool)
        .await
        .unwrap();
    Fx {
        tenant_id,
        project_id: boot.project_id,
        run_id,
        repository_id,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn worktree_and_mirror_have_distinct_ledger_identity(pool: PgPool) {
    let fx = seed(&pool, "PA08-WR-IDENTITY").await;

    let worktree_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.workspace_reclamation \
         (tenant_id, project_id, resource_kind, run_id, repository_id, relative_path) \
         VALUES ($1, $2, 'run_worktree', $3, $4, $5) RETURNING id",
    )
    .bind(fx.tenant_id)
    .bind(fx.project_id)
    .bind(fx.run_id)
    .bind(fx.repository_id)
    .bind(format!("repos/repo-{}", fx.repository_id))
    .fetch_one(&pool)
    .await
    .unwrap();

    let mirror_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.workspace_reclamation \
         (tenant_id, project_id, resource_kind, repository_id, relative_path) \
         VALUES ($1, $2, 'repository_mirror', $3, $4) RETURNING id",
    )
    .bind(fx.tenant_id)
    .bind(fx.project_id)
    .bind(fx.repository_id)
    .bind(format!("git-mirror/{}", fx.repository_id))
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_ne!(worktree_id, mirror_id);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.workspace_reclamation WHERE repository_id = $1",
    )
    .bind(fx.repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn ledger_rejects_duplicate_mirror_and_invalid_worktree_shape(pool: PgPool) {
    let fx = seed(&pool, "PA08-WR-CONSTRAINTS").await;

    sqlx::query(
        "INSERT INTO delivery.workspace_reclamation \
         (tenant_id, project_id, resource_kind, repository_id, relative_path) \
         VALUES ($1, $2, 'repository_mirror', $3, 'git-mirror/cache')",
    )
    .bind(fx.tenant_id)
    .bind(fx.project_id)
    .bind(fx.repository_id)
    .execute(&pool)
    .await
    .unwrap();

    let duplicate = sqlx::query(
        "INSERT INTO delivery.workspace_reclamation \
         (tenant_id, project_id, resource_kind, repository_id, relative_path) \
         VALUES ($1, $2, 'repository_mirror', $3, 'git-mirror/duplicate')",
    )
    .bind(fx.tenant_id)
    .bind(fx.project_id)
    .bind(fx.repository_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(matches!(duplicate, sqlx::Error::Database(_)));

    let invalid_shape = sqlx::query(
        "INSERT INTO delivery.workspace_reclamation \
         (tenant_id, project_id, resource_kind, repository_id, relative_path) \
         VALUES ($1, $2, 'run_worktree', $3, 'repos/missing-run')",
    )
    .bind(fx.tenant_id)
    .bind(fx.project_id)
    .bind(fx.repository_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(matches!(invalid_shape, sqlx::Error::Database(_)));
}


#[sqlx::test(migrations = "../../migrations")]
async fn worktree_candidate_requires_terminal_retention_and_completed_process_cleanup(pool: PgPool) {
    let fx = seed(&pool, "PA08-WR-WORKTREE-STAGE").await;
    sqlx::query(
        "UPDATE delivery.task_run \
            SET finished_at = now() - interval '31 days', executor_session = 'codex:pa08-cleanup' \
          WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        0,
        "a Run with an executor session but no completed process cleanup must not be staged"
    );

    sqlx::query(
        "INSERT INTO delivery.task_run_cleanup (run_id, session, executor_kind, status, completed_at) \
         VALUES ($1, 'codex:pa08-cleanup', 'codex', 'completed', now())",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        0,
        "candidate staging must be idempotent"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn archived_mirror_candidate_waits_for_all_run_cleanup(pool: PgPool) {
    let fx = seed(&pool, "PA08-WR-MIRROR-STAGE").await;
    sqlx::query(
        "UPDATE delivery.task_run SET finished_at = now() - interval '31 days' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE integration.project_repository \
            SET status = 'archived', updated_at = now() - interval '31 days' \
          WHERE id = $1",
    )
    .bind(fx.repository_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run_cleanup (run_id, session, executor_kind, status) \
         VALUES ($1, 'legacy-cleanup', 'codex', 'pending')",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        0,
        "unfinished process cleanup must block mirror staging"
    );

    sqlx::query(
        "UPDATE delivery.task_run_cleanup \
            SET status = 'completed', completed_at = now() WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        1,
        "worktree should be staged before mirror"
    );
    let worktree_adapter = FakeReclaimer::default();
    let worktree_report = pm_application::workspace_reclamation::run_worktree_reclamation_round(
        &pool, &worktree_adapter, 30,
    )
    .await
    .unwrap();
    assert_eq!(worktree_report.completed, 1);
    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        1,
        "mirror should be staged only after worktree completion"
    );
    assert!(
        pm_application::workspace_reclamation::claim_batch(&pool)
            .await
            .unwrap()
            .is_empty(),
        "worktree worker must not claim mirror candidates"
    );

    let row: (String, Option<Uuid>, String) = sqlx::query_as(
        "SELECT resource_kind, run_id, relative_path \
           FROM delivery.workspace_reclamation WHERE repository_id = $1",
    )
    .bind(fx.repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "repository_mirror");
    assert!(row.1.is_none());
    assert_eq!(row.2, format!("git-mirror/{}", fx.repository_id));
}


#[sqlx::test(migrations = "../../migrations")]
async fn reclamation_claims_are_fenced_and_revalidated_before_delete(pool: PgPool) {
    let fx = seed(&pool, "PA08-WR-FENCING").await;
    sqlx::query(
        "UPDATE delivery.task_run SET finished_at = now() - interval '31 days' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        1
    );

    let first = pm_application::workspace_reclamation::claim_batch(&pool)
        .await
        .unwrap();
    assert_eq!(first.len(), 1);
    let old = first[0].clone();
    assert!(
        pm_application::workspace_reclamation::revalidate_candidate(&pool, &old, 30)
            .await
            .unwrap()
    );

    sqlx::query(
        "UPDATE delivery.workspace_reclamation SET next_retry_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(old.id)
    .execute(&pool)
    .await
    .unwrap();
    let second = pm_application::workspace_reclamation::claim_batch(&pool)
        .await
        .unwrap();
    assert_eq!(second.len(), 1);
    let current = second[0].clone();
    assert_ne!(old.claim_token, current.claim_token);

    let old_token = old.claim_token.unwrap();
    let current_token = current.claim_token.unwrap();
    assert!(
        !pm_application::workspace_reclamation::mark_completed(&pool, old.id, old_token)
            .await
            .unwrap(),
        "stale claimant cannot complete a newer claim"
    );
    assert!(
        !pm_application::workspace_reclamation::revalidate_candidate(&pool, &old, 30)
            .await
            .unwrap(),
        "stale claimant must fail the second eligibility check"
    );
    assert!(
        pm_application::workspace_reclamation::revalidate_candidate(&pool, &current, 30)
            .await
            .unwrap()
    );
    assert!(
        pm_application::workspace_reclamation::mark_failed(
            &pool,
            current.id,
            current_token,
            "simulated filesystem failure",
            60,
        )
        .await
        .unwrap()
    );
}


#[derive(Default)]
struct FakeReclaimer {
    calls: AtomicUsize,
    mirror_calls: AtomicUsize,
}

#[async_trait::async_trait]
impl pm_application::workspace_reclamation::WorkspaceReclaimer for FakeReclaimer {
    async fn reclaim_run_worktree(
        &self,
        _run_id: Uuid,
        _repository_id: Uuid,
        _workspace_path: &str,
    ) -> Result<(), pm_application::run_dispatch::ExecutorError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn reclaim_repository_mirror(
        &self,
        _repository_id: Uuid,
    ) -> Result<(), pm_application::run_dispatch::ExecutorError> {
        self.mirror_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

async fn stage_mirror_candidate(pool: &PgPool, fx: &Fx) {
    sqlx::query(
        "UPDATE delivery.task_run SET finished_at = now() - interval '31 days' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE integration.project_repository \
            SET status = 'archived', updated_at = now() - interval '31 days' \
          WHERE id = $1",
    )
    .bind(fx.repository_id)
    .execute(pool)
    .await
    .unwrap();

    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(pool, 30)
            .await
            .unwrap(),
        1,
        "the old Run worktree must be staged before the mirror"
    );
    let adapter = FakeReclaimer::default();
    let worktree_report = pm_application::workspace_reclamation::run_worktree_reclamation_round(
        pool, &adapter, 30,
    )
    .await
    .unwrap();
    assert_eq!(worktree_report.completed, 1);
    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(pool, 30)
            .await
            .unwrap(),
        1,
        "the mirror becomes eligible only after the Run worktree is completed"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn worktree_reclamation_round_marks_success_only_after_adapter_returns(pool: PgPool) {
    let fx = seed(&pool, "PA08-WR-ROUND-SUCCESS").await;
    sqlx::query(
        "UPDATE delivery.task_run SET finished_at = now() - interval '31 days' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        1
    );

    let adapter = FakeReclaimer::default();
    let report = pm_application::workspace_reclamation::run_worktree_reclamation_round(
        &pool, &adapter, 30,
    )
    .await
    .unwrap();
    assert_eq!(report.claimed, 1);
    assert_eq!(report.completed, 1);
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delivery.workspace_reclamation WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn worktree_reclamation_round_skips_candidate_that_fails_second_check(pool: PgPool) {
    let fx = seed(&pool, "PA08-WR-ROUND-RECHECK").await;
    sqlx::query(
        "UPDATE delivery.task_run SET finished_at = now() - interval '31 days' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        1
    );
    sqlx::query("UPDATE delivery.task_run SET finished_at = now() WHERE id = $1")
        .bind(fx.run_id)
        .execute(&pool)
        .await
        .unwrap();

    let adapter = FakeReclaimer::default();
    let report = pm_application::workspace_reclamation::run_worktree_reclamation_round(
        &pool, &adapter, 30,
    )
    .await
    .unwrap();
    assert_eq!(report.claimed, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
}


#[sqlx::test(migrations = "../../migrations")]
async fn mirror_reclamation_round_rechecks_archive_state_under_repository_lock(pool: PgPool) {
    let fx = seed(&pool, "mirror_reclamation_round_rechecks_archive_state_under_repository_lock").await;
    stage_mirror_candidate(&pool, &fx).await;

    let adapter = FakeReclaimer::default();
    let report = pm_application::workspace_reclamation::run_repository_mirror_reclamation_round(
        &pool, &adapter, 30,
    )
    .await
    .unwrap();
    assert_eq!(report.claimed, 1);
    assert_eq!(report.completed, 1);
    assert_eq!(adapter.mirror_calls.load(Ordering::SeqCst), 1);

    let status: String = sqlx::query_scalar(
        "SELECT status FROM delivery.workspace_reclamation WHERE repository_id = $1",
    )
    .bind(fx.repository_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
}

struct RepositoryLockProbe {
    pool: PgPool,
    repository_id: Uuid,
    blocked: AtomicBool,
    updated: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl pm_application::workspace_reclamation::WorkspaceReclaimer for RepositoryLockProbe {
    async fn reclaim_run_worktree(
        &self,
        _run_id: Uuid,
        _repository_id: Uuid,
        _workspace_path: &str,
    ) -> Result<(), pm_application::run_dispatch::ExecutorError> {
        unreachable!("mirror round must not call worktree reclamation")
    }

    async fn reclaim_repository_mirror(
        &self,
        _repository_id: Uuid,
    ) -> Result<(), pm_application::run_dispatch::ExecutorError> {
        let pool = self.pool.clone();
        let repository_id = self.repository_id;
        let updated = Arc::clone(&self.updated);
        let update = tokio::spawn(async move {
            let result = sqlx::query(
                "UPDATE integration.project_repository SET status = 'verified' WHERE id = $1",
            )
            .bind(repository_id)
            .execute(&pool)
            .await;
            updated.store(result.is_ok(), Ordering::SeqCst);
            result
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        self.blocked.store(!update.is_finished(), Ordering::SeqCst);
        Ok(())
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn mirror_reclamation_holds_repository_row_lock_through_filesystem_adapter(pool: PgPool) {
    let fx = seed(&pool, "mirror_reclamation_holds_repository_row_lock_through_filesystem_adapter").await;
    stage_mirror_candidate(&pool, &fx).await;

    let adapter = RepositoryLockProbe {
        pool: pool.clone(),
        repository_id: fx.repository_id,
        blocked: AtomicBool::new(false),
        updated: Arc::new(AtomicBool::new(false)),
    };
    let report = pm_application::workspace_reclamation::run_repository_mirror_reclamation_round(
        &pool, &adapter, 30,
    )
    .await
    .unwrap();
    assert_eq!(report.completed, 1);
    assert!(
        adapter.blocked.load(Ordering::SeqCst),
        "Repository reactivation must block until mirror deletion transaction commits"
    );
    for _ in 0..20 {
        if adapter.updated.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(adapter.updated.load(Ordering::SeqCst), "blocked reactivation should proceed after commit");
}


#[sqlx::test(migrations = "../../migrations")]
async fn reclamation_needs_human_can_be_listed_and_explicitly_requeued(pool: PgPool) {
    let fx = seed(&pool, "PA08-WR-NEEDS-HUMAN").await;
    sqlx::query(
        "UPDATE delivery.task_run SET finished_at = now() - interval '31 days' WHERE id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        pm_application::workspace_reclamation::stage_candidates(&pool, 30)
            .await
            .unwrap(),
        1
    );
    sqlx::query(
        "UPDATE delivery.workspace_reclamation SET max_attempts = 1 WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await
    .unwrap();

    let claimed = pm_application::workspace_reclamation::claim_batch(&pool)
        .await
        .unwrap();
    let token = claimed[0].claim_token.unwrap();
    assert!(
        pm_application::workspace_reclamation::mark_failed(
            &pool,
            claimed[0].id,
            token,
            "operator review required",
            60,
        )
        .await
        .unwrap()
    );

    let needs_human = pm_application::workspace_reclamation::list_needs_human(&pool, 10)
        .await
        .unwrap();
    assert_eq!(needs_human.len(), 1);
    assert_eq!(needs_human[0].id, claimed[0].id);
    assert_eq!(needs_human[0].status, "needs_human");

    assert!(
        pm_application::workspace_reclamation::requeue_needs_human(&pool, claimed[0].id)
            .await
            .unwrap()
    );
    let status: (String, i32, Option<Uuid>) = sqlx::query_as(
        "SELECT status, attempts, claim_token FROM delivery.workspace_reclamation WHERE id = $1",
    )
    .bind(claimed[0].id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status.0, "pending");
    assert_eq!(status.1, 0);
    assert!(status.2.is_none());
}


#[sqlx::test(migrations = "../../migrations")]
async fn mirror_reclamation_skips_repository_reactivated_after_candidate_staging(pool: PgPool) {
    let fx = seed(&pool, "mirror_reclamation_skips_repository_reactivated_after_candidate_staging").await;
    stage_mirror_candidate(&pool, &fx).await;
    sqlx::query(
        "UPDATE integration.project_repository SET status = 'verified', updated_at = now() WHERE id = $1",
    )
    .bind(fx.repository_id)
    .execute(&pool)
    .await
    .unwrap();

    let adapter = FakeReclaimer::default();
    let report = pm_application::workspace_reclamation::run_repository_mirror_reclamation_round(
        &pool, &adapter, 30,
    )
    .await
    .unwrap();
    assert_eq!(report.claimed, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(adapter.mirror_calls.load(Ordering::SeqCst), 0);
}
