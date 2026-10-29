//! Lease-based job queue over `integration.job` (architecture doc §11.2).
//!
//! The claim path is a single CTE statement: candidate selection with
//! `FOR UPDATE SKIP LOCKED`, then an atomic lease grant. The freeze filter
//! lives here in the claim layer — a job whose project is disabled or not
//! in `ai_lead` mode is never handed out, regardless of its queue state.

use pm_domain::{EventId, ProjectId};

/// A job handed to a worker, with its lease granted.
pub struct ClaimedJob {
    pub job_id: uuid::Uuid,
    pub event_id: EventId,
    pub project_id: ProjectId,
}

/// Claim one claimable job of `job_type` for `worker_id`, granting a lease
/// of `lease_secs`. Returns `None` when nothing is claimable.
///
/// Candidate rows are either due queued jobs (`status = 'queued'` and
/// `available_at <= now()`) or expired leases (`status = 'running'` and
/// `lease_until < now()`) — claiming is itself the reclaim path for a
/// crashed worker, so no separate reaper is needed. In both cases the
/// owning project's AI config must be enabled and in `ai_lead` mode
/// (frozen / `controlled` projects yield no candidates). `SKIP LOCKED`
/// keeps concurrent workers off each other's rows; claiming also
/// (re)grants the lease, so a claimed job cannot be handed out twice
/// within its lease.
pub async fn claim(
    pool: &sqlx::PgPool,
    job_type: &str,
    worker_id: &str,
    lease_secs: i64,
) -> Option<ClaimedJob> {
    // (job.id, job.event_id, job.project_id), positional.
    sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, uuid::Uuid)>(
        r#"
        WITH candidate AS (
            SELECT job.id
            FROM integration.job job
            JOIN ai.project_ai_config cfg
              ON cfg.project_id = job.project_id
             AND cfg.enabled
             AND cfg.automation_mode = 'ai_lead'
            WHERE job.job_type = $1
              AND (
                    (job.status = 'queued' AND job.available_at <= now())
                 OR (job.status = 'running' AND job.lease_until < now())
                  )
            ORDER BY job.available_at
            FOR UPDATE OF job SKIP LOCKED
            LIMIT 1
        )
        UPDATE integration.job job
        SET status = 'running',
            lease_owner = $2,
            lease_until = now() + make_interval(secs => $3::float8)
        FROM candidate
        WHERE job.id = candidate.id
        RETURNING job.id, job.event_id, job.project_id
        "#,
    )
    .bind(job_type)
    .bind(worker_id)
    .bind(lease_secs)
    .fetch_optional(pool)
    .await
    .ok()?
    .map(|(job_id, event_id, project_id)| ClaimedJob {
        job_id,
        event_id,
        project_id,
    })
}

/// Mark a job completed and drop its lease.
pub async fn complete(pool: &sqlx::PgPool, job_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE integration.job \
         SET status = 'completed', lease_owner = NULL, lease_until = NULL \
         WHERE id = $1",
    )
    .bind(job_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Record a failure with exponential backoff.
///
/// Increments `attempts` and stores `error` as `last_error`. Once
/// `attempts` reaches `max_attempts` the job is terminally `failed`;
/// otherwise it returns to `queued` with
/// `available_at = now() + backoff_base_secs * 2^attempts` (the
/// post-increment attempt count) and its lease cleared, so the next
/// retry waits out the backoff.
pub async fn fail(
    pool: &sqlx::PgPool,
    job_id: uuid::Uuid,
    error: &str,
    max_attempts: i32,
    backoff_base_secs: i64,
) -> Result<(), sqlx::Error> {
    // In UPDATE SET expressions `attempts` is the pre-increment value, so
    // `attempts + 1` is both the stored count and the backoff exponent.
    sqlx::query(
        r#"
        UPDATE integration.job
        SET attempts = attempts + 1,
            last_error = $2,
            lease_owner = NULL,
            lease_until = NULL,
            status = CASE WHEN attempts + 1 >= $3 THEN 'failed' ELSE 'queued' END,
            available_at = CASE
                WHEN attempts + 1 >= $3 THEN available_at
                ELSE now() + make_interval(
                    secs => $4::float8 * power(2.0, (attempts + 1)::float8))
            END
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(error)
    .bind(max_attempts)
    .bind(backoff_base_secs)
    .execute(pool)
    .await
    .map(|_| ())
}
