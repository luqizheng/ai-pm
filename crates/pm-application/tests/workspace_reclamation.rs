//! PA08-04 schema-level tests for the independent workspace reclamation ledger.

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
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
        1
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
