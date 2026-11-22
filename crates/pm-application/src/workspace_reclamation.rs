//! PA08-04 candidate staging for derived local workspace resources.
//!
//! This module only stages database candidates. It does not delete files. The
//! filesystem worker must perform a second eligibility check under the shared
//! repository mirror lock before any destructive operation.

use sqlx::PgPool;

/// Conservative default retention for terminal Run worktrees and archived mirrors.
pub const DEFAULT_RETENTION_DAYS: i32 = 30;

/// Idempotently stage eligible Run worktrees and archived repository mirrors.
///
/// A worktree is staged only after its Run has been terminal for the retention
/// period and any recorded executor process cleanup is completed. A mirror is
/// staged only after its Repository has been archived for the retention period
/// and no active Run or unfinished process cleanup references it.
///
/// Staging a candidate is metadata-only and never removes filesystem content.
pub async fn stage_candidates(
    pool: &PgPool,
    retention_days: i32,
) -> Result<u64, sqlx::Error> {
    if retention_days < 0 {
        return Err(sqlx::Error::Protocol(
            "workspace reclamation retention_days cannot be negative".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let worktrees = sqlx::query(
        "INSERT INTO delivery.workspace_reclamation \
         (tenant_id, project_id, resource_kind, run_id, repository_id, relative_path) \
         SELECT r.tenant_id, r.project_id, 'run_worktree', r.id, rr.repository_id, rr.workspace_path \
           FROM delivery.task_run r \
           JOIN delivery.task_run_repository rr ON rr.run_id = r.id \
          WHERE r.run_kind = 'delivery' \
            AND r.status IN ('succeeded', 'failed', 'timed_out', 'cancelled') \
            AND r.finished_at IS NOT NULL \
            AND r.finished_at <= now() - make_interval(days => $1) \
            AND (r.executor_session IS NULL OR EXISTS ( \
                SELECT 1 FROM delivery.task_run_cleanup cleanup \
                 WHERE cleanup.run_id = r.id AND cleanup.status = 'completed' \
            )) \
            AND NOT EXISTS ( \
                SELECT 1 FROM delivery.task_run_cleanup cleanup \
                 WHERE cleanup.run_id = r.id AND cleanup.status <> 'completed' \
            ) \
         ON CONFLICT DO NOTHING",
    )
    .bind(retention_days)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let mirrors = sqlx::query(
        "INSERT INTO delivery.workspace_reclamation \
         (tenant_id, project_id, resource_kind, repository_id, relative_path) \
         SELECT pr.tenant_id, pr.project_id, 'repository_mirror', pr.id, \
                'git-mirror/' || pr.id::text \
           FROM integration.project_repository pr \
          WHERE pr.status = 'archived' \
            AND pr.updated_at <= now() - make_interval(days => $1) \
            AND NOT EXISTS ( \
                SELECT 1 \
                  FROM delivery.task_run_repository rr \
                  JOIN delivery.task_run r ON r.id = rr.run_id \
                 WHERE rr.repository_id = pr.id \
                   AND ( \
                       r.status NOT IN ('succeeded', 'failed', 'timed_out', 'cancelled') \
                       OR (r.executor_session IS NOT NULL AND NOT EXISTS ( \
                           SELECT 1 FROM delivery.task_run_cleanup cleanup \
                            WHERE cleanup.run_id = r.id AND cleanup.status = 'completed' \
                       )) \
                       OR EXISTS ( \
                           SELECT 1 FROM delivery.task_run_cleanup cleanup \
                            WHERE cleanup.run_id = r.id AND cleanup.status <> 'completed' \
                       ) \
                   ) \
            ) \
         ON CONFLICT DO NOTHING",
    )
    .bind(retention_days)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await?;
    Ok(worktrees + mirrors)
}
