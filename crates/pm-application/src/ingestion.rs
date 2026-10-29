//! Idempotent inbound event ingestion (Task 9): the write side of the inbox.
//!
//! [`submit_event`] runs in a single transaction:
//!
//! 1. `INSERT INTO integration.inbound_event ...` with
//!    `payload_hash = SHA-256(payload)` (lowercase hex) and
//!    `ON CONFLICT (project_id, source, idempotency_key) DO NOTHING`,
//! 2. on the happy path, one `integration.job` row
//!    (`job_type = 'ai_analysis'`, status `queued`) pointing at the new event.
//!
//! On conflict the original row is read back: same `payload_hash` →
//! [`SubmitOutcome::Duplicate`] with the original event id and status (no
//! second job — the event was already enqueued once); different hash →
//! [`IngestionError::IdempotencyConflict`] naming the existing event. Foreign-key
//! violations (unknown project or unknown actor) map to
//! [`IngestionError::UnknownProject`]: the brief's error enum has no
//! actor-specific variant, so the unknown actor is carried there too.
//!
//! The payload fingerprint is taken over `serde_json::Value`'s canonical
//! serialization (`to_string`, BTreeMap key order), so two submissions carrying
//! equal JSON values hash identically regardless of original key order.

use pm_domain::{EventId, PrincipalId, ProjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

/// One inbound event as accepted from any channel (Task 16 MCP tools included).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitEventRequest {
    pub project_id: ProjectId,
    pub actor_id: PrincipalId,
    pub source: String,
    pub event_type: String,
    pub idempotency_key: String,
    pub payload: serde_json::Value,
}

/// What happened to the submission: fresh insert or idempotent replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitOutcome {
    /// New event row plus its `ai_analysis` job, both committed.
    Accepted { event_id: EventId },
    /// Same key, same payload: the pre-existing event and its current status.
    Duplicate { event_id: EventId, status: String },
}

#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error(
        "idempotency conflict: key already used with a different payload (existing event {existing_event_id})"
    )]
    IdempotencyConflict { existing_event_id: EventId },
    #[error("unknown project or actor")]
    UnknownProject,
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// Inserts the event plus its `ai_analysis` job in one transaction, with
/// `(project_id, source, idempotency_key)` idempotency (see module docs).
pub async fn submit_event(
    pool: &PgPool,
    req: SubmitEventRequest,
) -> Result<SubmitOutcome, IngestionError> {
    let payload_hash = sha256_hex(&req.payload.to_string());

    let mut tx = pool.begin().await.map_err(IngestionError::Db)?;

    let inserted: Option<(EventId, String)> = sqlx::query_as(
        "INSERT INTO integration.inbound_event \
         (project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (project_id, source, idempotency_key) DO NOTHING \
         RETURNING id, status",
    )
    .bind(req.project_id)
    .bind(req.actor_id)
    .bind(&req.source)
    .bind(&req.event_type)
    .bind(&req.idempotency_key)
    .bind(&req.payload)
    .bind(&payload_hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(fk_to_unknown_project)?;

    match inserted {
        Some((event_id, _)) => {
            sqlx::query(
                "INSERT INTO integration.job (project_id, event_id, job_type) \
                 VALUES ($1, $2, 'ai_analysis')",
            )
            .bind(req.project_id)
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(fk_to_unknown_project)?;
            tx.commit().await.map_err(IngestionError::Db)?;
            Ok(SubmitOutcome::Accepted { event_id })
        }
        None => {
            // Key already taken: read the original row back to decide between
            // an idempotent replay and a genuine conflict. The row must exist —
            // a concurrent inserter blocks this INSERT until it commits.
            let existing: Option<(EventId, String, String)> = sqlx::query_as(
                "SELECT id, payload_hash, status FROM integration.inbound_event \
                 WHERE project_id = $1 AND source = $2 AND idempotency_key = $3",
            )
            .bind(req.project_id)
            .bind(&req.source)
            .bind(&req.idempotency_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(IngestionError::Db)?;
            match existing {
                Some((event_id, hash, status)) if hash == payload_hash => {
                    tx.commit().await.map_err(IngestionError::Db)?;
                    Ok(SubmitOutcome::Duplicate { event_id, status })
                }
                Some((event_id, _, _)) => {
                    Err(IngestionError::IdempotencyConflict { existing_event_id: event_id })
                }
                None => unreachable!(
                    "ON CONFLICT DO NOTHING fired but the conflicting row is not readable"
                ),
            }
        }
    }
}

/// Foreign-key violations on the event insert (unknown project or unknown
/// actor) surface as [`IngestionError::UnknownProject`]; everything else stays
/// a plain [`IngestionError::Db`].
fn fk_to_unknown_project(err: sqlx::Error) -> IngestionError {
    match &err {
        sqlx::Error::Database(db) if db.is_foreign_key_violation() => IngestionError::UnknownProject,
        _ => IngestionError::Db(err),
    }
}

/// Lowercase hex SHA-256 over the given bytes.
fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}
