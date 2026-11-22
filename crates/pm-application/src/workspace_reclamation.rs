//! PA08-04 candidate staging and fenced lifecycle for derived local workspace resources.
//!
//! Candidate discovery and ledger transitions are database-only. Filesystem
//! deletion is performed by the infrastructure adapter only after a claim has
//! been revalidated immediately before the operation.

use pm_domain::Uuid;
use sqlx::PgPool;

pub const DEFAULT_RETENTION_DAYS: i32 = 30;
const CLAIM_LEASE_SECS: f64 = 300.0;
const CLAIM_BATCH_LIMIT: i64 = 32;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReclamationEntry {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub resource_kind: String,
    pub run_id: Option<Uuid>,
    pub repository_id: Uuid,
    pub relative_path: String,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub next_retry_at: chrono::DateTime<chrono::Utc>,
    pub last_error: Option<String>,
    pub claim_token: Option<Uuid>,
}

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

/// Atomically claim due candidates and fence each claim with a fresh token.
/// The same timestamp column is the retry due time while pending and the lease
/// expiry while cleaning.
pub async fn claim_batch(pool: &PgPool) -> Result<Vec<ReclamationEntry>, sqlx::Error> {
    sqlx::query_as::<_, ReclamationEntry>(
        "WITH candidates AS ( \
             SELECT id FROM delivery.workspace_reclamation \
              WHERE status IN ('pending', 'cleaning') AND next_retry_at <= now() \
              ORDER BY next_retry_at, created_at \
              LIMIT $1 FOR UPDATE SKIP LOCKED \
         ) \
         UPDATE delivery.workspace_reclamation AS item \
            SET status = 'cleaning', claim_token = gen_random_uuid(), \
                next_retry_at = now() + make_interval(secs => $2::float8), updated_at = now() \
           FROM candidates WHERE item.id = candidates.id \
         RETURNING item.id, item.tenant_id, item.project_id, item.resource_kind, \
                   item.run_id, item.repository_id, item.relative_path, item.status, \
                   item.attempts, item.max_attempts, item.next_retry_at, item.last_error, item.claim_token",
    )
    .bind(CLAIM_BATCH_LIMIT)
    .bind(CLAIM_LEASE_SECS)
    .fetch_all(pool)
    .await
}

/// Second-check every eligibility predicate immediately before filesystem work.
/// The current claim token must still own the ledger row.
pub async fn revalidate_candidate(
    pool: &PgPool,
    entry: &ReclamationEntry,
    retention_days: i32,
) -> Result<bool, sqlx::Error> {
    if retention_days < 0 {
        return Err(sqlx::Error::Protocol(
            "workspace reclamation retention_days cannot be negative".into(),
        ));
    }
    let token = entry.claim_token.ok_or_else(|| {
        sqlx::Error::Protocol("reclamation claim is missing its fencing token".into())
    })?;
    match entry.resource_kind.as_str() {
        "run_worktree" => {
            let run_id = entry.run_id.ok_or_else(|| {
                sqlx::Error::Protocol("run_worktree candidate is missing run_id".into())
            })?;
            sqlx::query_scalar(
                "SELECT EXISTS ( \
                    SELECT 1 \
                      FROM delivery.workspace_reclamation wr \
                      JOIN delivery.task_run r ON r.id = wr.run_id \
                      JOIN delivery.task_run_repository rr \
                        ON rr.run_id = r.id AND rr.repository_id = wr.repository_id \
                     WHERE wr.id = $1 AND wr.run_id = $2 AND wr.claim_token = $3 \
                       AND wr.status = 'cleaning' AND wr.resource_kind = 'run_worktree' \
                       AND wr.relative_path = rr.workspace_path \
                       AND r.run_kind = 'delivery' \
                       AND r.status IN ('succeeded', 'failed', 'timed_out', 'cancelled') \
                       AND r.finished_at IS NOT NULL \
                       AND r.finished_at <= now() - make_interval(days => $4) \
                       AND (r.executor_session IS NULL OR EXISTS ( \
                           SELECT 1 FROM delivery.task_run_cleanup cleanup \
                            WHERE cleanup.run_id = r.id AND cleanup.status = 'completed' \
                       )) \
                       AND NOT EXISTS ( \
                           SELECT 1 FROM delivery.task_run_cleanup cleanup \
                            WHERE cleanup.run_id = r.id AND cleanup.status <> 'completed' \
                       ) \
                )",
            )
            .bind(entry.id)
            .bind(run_id)
            .bind(token)
            .bind(retention_days)
            .fetch_one(pool)
            .await
        }
        "repository_mirror" => {
            sqlx::query_scalar(
                "SELECT EXISTS ( \
                    SELECT 1 \
                      FROM delivery.workspace_reclamation wr \
                      JOIN integration.project_repository pr \
                        ON pr.id = wr.repository_id AND pr.project_id = wr.project_id \
                     WHERE wr.id = $1 AND wr.claim_token = $2 \
                       AND wr.status = 'cleaning' AND wr.resource_kind = 'repository_mirror' \
                       AND wr.run_id IS NULL \
                       AND wr.relative_path = 'git-mirror/' || pr.id::text \
                       AND pr.status = 'archived' \
                       AND pr.updated_at <= now() - make_interval(days => $3) \
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
                )",
            )
            .bind(entry.id)
            .bind(token)
            .bind(retention_days)
            .fetch_one(pool)
            .await
        }
        other => Err(sqlx::Error::Protocol(
            format!("unknown workspace reclamation resource kind: {other}").into(),
        )),
    }
}

pub async fn mark_completed(
    pool: &PgPool,
    id: Uuid,
    claim_token: Uuid,
) -> Result<bool, sqlx::Error> {
    let rows = sqlx::query(
        "UPDATE delivery.workspace_reclamation \
            SET status = 'completed', completed_at = now(), claim_token = NULL, \
                last_error = NULL, updated_at = now() \
          WHERE id = $1 AND status = 'cleaning' AND claim_token = $2",
    )
    .bind(id)
    .bind(claim_token)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows == 1)
}

pub async fn mark_failed(
    pool: &PgPool,
    id: Uuid,
    claim_token: Uuid,
    error: &str,
    backoff_secs: i64,
) -> Result<bool, sqlx::Error> {
    let rows = sqlx::query(
        "UPDATE delivery.workspace_reclamation \
            SET attempts = attempts + 1, \
                status = CASE WHEN attempts + 1 >= max_attempts THEN 'needs_human' ELSE 'pending' END, \
                next_retry_at = now() + make_interval(secs => $3::float8), \
                claim_token = NULL, last_error = $4, updated_at = now() \
          WHERE id = $1 AND status = 'cleaning' AND claim_token = $2",
    )
    .bind(id)
    .bind(claim_token)
    .bind(backoff_secs as f64)
    .bind(truncate_error(error))
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows == 1)
}

fn truncate_error(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.chars().count() <= 480 {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(480).collect();
    out.push('…');
    out
}
