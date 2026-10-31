//! Transactional outbox over `integration.outbox_event`
//! (task state machine spec §20.2–§20.6).
//!
//! `append` only accepts a `&mut Transaction` — the signature makes the
//! "business commit landed, event lost" failure mode unrepresentable.
//! Delivery is at-least-once: a lease that expires (crashed worker) is
//! re-handed out as the same row, so handlers must be idempotent; the
//! `idempotency_key` UNIQUE constraint collapses duplicate semantic
//! events at write time. Failures retry with exponential backoff until
//! `max_attempts`, then dead-letter.

/// Attempt ceiling for outbox delivery before an event dead-letters.
pub const OUTBOX_MAX_ATTEMPTS: i32 = 5;
/// Base for exponential backoff: `next_attempt_at = now() + base * 2^attempts`.
pub const OUTBOX_BACKOFF_BASE_SECS: i64 = 5;
/// Default lease duration for a claimed event.
pub const OUTBOX_LEASE_SECS: i64 = 60;

/// An event to persist inside the caller's business transaction.
pub struct NewOutboxEvent<'a> {
    pub project_id: uuid::Uuid,
    pub event_type: &'a str,
    pub aggregate_type: &'a str,
    pub aggregate_id: &'a str,
    pub payload: serde_json::Value,
    pub idempotency_key: &'a str,
}

/// An event handed to a worker, with its lease granted.
pub struct ClaimedOutboxEvent {
    pub id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: serde_json::Value,
    pub attempts: i32,
    pub schema_version: i32,
    pub idempotency_key: String,
}

/// Result of `fail`: the event was either queued for a backoff retry or
/// terminally dead-lettered.
#[derive(Debug, PartialEq, Eq)]
pub enum OutboxFailOutcome {
    Retried,
    DeadLettered,
}

/// Insert an event as part of the caller's transaction. The row only
/// becomes visible when that transaction commits — a rollback takes the
/// event with it.
pub async fn append(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: NewOutboxEvent<'_>,
) -> Result<uuid::Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO integration.outbox_event \
         (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(event.project_id)
    .bind(event.event_type)
    .bind(event.aggregate_type)
    .bind(event.aggregate_id)
    .bind(event.payload)
    .bind(event.idempotency_key)
    .fetch_one(&mut **tx)
    .await
}

/// Claim one due event for `worker_id`, granting a lease of `lease_secs`.
/// Returns `None` when nothing is claimable.
///
/// Candidate rows are either due pending events (`status = 'pending'`
/// and `next_attempt_at <= now()`) or expired leases (`status =
/// 'processing'` and `locked_until < now()`) — claiming is itself the
/// reclaim path for a crashed worker, so no separate reaper is needed,
/// and redelivery reuses the same row (at-least-once). `SKIP LOCKED`
/// keeps concurrent workers off each other's rows; claiming counts the
/// attempt, so a claimed event cannot be handed out again within its
/// lease.
pub async fn claim(
    pool: &sqlx::PgPool,
    worker_id: &str,
    lease_secs: i64,
) -> Option<ClaimedOutboxEvent> {
    sqlx::query_as::<
        _,
        (
            uuid::Uuid,
            uuid::Uuid,
            String,
            String,
            String,
            serde_json::Value,
            i32,
            i32,
            String,
        ),
    >(
        r#"
        WITH candidate AS (
            SELECT event.id
            FROM integration.outbox_event event
            WHERE (
                  (event.status = 'pending' AND event.next_attempt_at <= now())
               OR (event.status = 'processing' AND event.locked_until < now())
                  )
            ORDER BY event.next_attempt_at
            FOR UPDATE OF event SKIP LOCKED
            LIMIT 1
        )
        UPDATE integration.outbox_event event
        SET status = 'processing',
            attempts = attempts + 1,
            locked_until = now() + make_interval(secs => $2::float8),
            lease_owner = $1
        FROM candidate
        WHERE event.id = candidate.id
        RETURNING event.id, event.project_id, event.event_type, event.aggregate_type,
                  event.aggregate_id, event.payload, event.attempts,
                  event.schema_version, event.idempotency_key
        "#,
    )
    .bind(worker_id)
    .bind(lease_secs)
    .fetch_optional(pool)
    .await
    .ok()?
    .map(
        |(
            id,
            project_id,
            event_type,
            aggregate_type,
            aggregate_id,
            payload,
            attempts,
            schema_version,
            idempotency_key,
        )| {
            ClaimedOutboxEvent {
                id,
                project_id,
                event_type,
                aggregate_type,
                aggregate_id,
                payload,
                attempts,
                schema_version,
                idempotency_key,
            }
        },
    )
}

/// Mark an event delivered and drop its lease.
pub async fn complete(pool: &sqlx::PgPool, id: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE integration.outbox_event \
         SET status = 'delivered', delivered_at = now(), \
             lease_owner = NULL, locked_until = NULL \
         WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Record a delivery failure with exponential backoff.
///
/// Stores `error` as `last_error` and clears the lease. Once `attempts`
/// (already counted by `claim`) reaches `max_attempts` the event is
/// terminally `dead_letter`; otherwise it returns to `pending` with
/// `next_attempt_at = now() + backoff_base_secs * 2^attempts`, so the
/// next retry waits out the backoff.
pub async fn fail(
    pool: &sqlx::PgPool,
    id: uuid::Uuid,
    error: &str,
    max_attempts: i32,
    backoff_base_secs: i64,
) -> Result<OutboxFailOutcome, sqlx::Error> {
    let status: String = sqlx::query_scalar(
        r#"
        UPDATE integration.outbox_event
        SET last_error = $2,
            lease_owner = NULL,
            locked_until = NULL,
            status = CASE WHEN attempts >= $3 THEN 'dead_letter' ELSE 'pending' END,
            next_attempt_at = CASE
                WHEN attempts >= $3 THEN next_attempt_at
                ELSE now() + make_interval(
                    secs => $4::float8 * power(2.0, attempts::float8))
            END
        WHERE id = $1
        RETURNING status
        "#,
    )
    .bind(id)
    .bind(error)
    .bind(max_attempts)
    .bind(backoff_base_secs)
    .fetch_one(pool)
    .await?;
    Ok(match status.as_str() {
        "dead_letter" => OutboxFailOutcome::DeadLettered,
        _ => OutboxFailOutcome::Retried,
    })
}