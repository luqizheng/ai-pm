//! Idempotent inbound event ingestion tests (Task 9, Step 1).
//!
//! Contract under test: `submit_event` runs the event insert plus the
//! `ai_analysis` job insert in one transaction, deduplicates on
//! `(project_id, source, idempotency_key)` (same payload → `Duplicate` with the
//! original event id and no second job; different payload → `IdempotencyConflict`
//! carrying the existing event id) and maps foreign-key violations on the
//! unknown project / unknown actor to `IngestionError::UnknownProject`.
//!
//! Projects are seeded with `bootstrap_project` (Task 8), which provides the
//! `core.project` row and the three principals the event FKs point at.

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::ingestion::{IngestionError, SubmitEventRequest, SubmitOutcome, submit_event};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

fn seed(code: &str) -> BootstrapRequest {
    BootstrapRequest {
        project_code: code.to_string(),
        project_name: format!("project {code}"),
        tenant_id: SYSTEM_TENANT,
    }
}

fn submit_req(
    project_id: Uuid,
    actor_id: Uuid,
    idempotency_key: &str,
    payload: Value,
) -> SubmitEventRequest {
    SubmitEventRequest {
        project_id,
        actor_id,
        source: "feishu".to_string(),
        event_type: "bug_report".to_string(),
        idempotency_key: idempotency_key.to_string(),
        payload,
    }
}

/// The hash the service must persist: lowercase hex SHA-256 over the canonical
/// `serde_json` serialization of the payload.
fn expected_hash(payload: &Value) -> String {
    let digest = Sha256::digest(payload.to_string().as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Step 1 test 1: the first submission is `Accepted`, creates exactly one
/// `integration.inbound_event` row (status `received`, payload_hash = SHA-256
/// hex of the payload) and exactly one `integration.job` row
/// (`job_type = 'ai_analysis'`, status `queued`).
#[sqlx::test(migrations = "../../migrations")]
async fn accepted_submission_creates_event_and_job(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("ING-1")).await.unwrap();
    let payload = json!({"text": "login returns 500", "channel": "#support"});
    let req = submit_req(boot.project_id, boot.human_id, "feishu-msg-1", payload.clone());

    let outcome = submit_event(&pool, req).await.unwrap();
    let SubmitOutcome::Accepted { event_id } = outcome else {
        panic!("expected SubmitOutcome::Accepted, got {outcome:?}")
    };

    let event = sqlx::query(
        "SELECT project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, status \
         FROM integration.inbound_event WHERE id = $1",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event.get::<Uuid, _>("project_id"), boot.project_id);
    assert_eq!(event.get::<Uuid, _>("actor_id"), boot.human_id);
    assert_eq!(event.get::<String, _>("source"), "feishu");
    assert_eq!(event.get::<String, _>("event_type"), "bug_report");
    assert_eq!(event.get::<String, _>("idempotency_key"), "feishu-msg-1");
    assert_eq!(event.get::<Value, _>("payload"), payload);
    assert_eq!(event.get::<String, _>("payload_hash"), expected_hash(&payload));
    assert_eq!(event.get::<String, _>("status"), "received");

    let jobs: Vec<(String, String)> = sqlx::query_as(
        "SELECT job_type, status FROM integration.job WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(jobs, vec![("ai_analysis".to_string(), "queued".to_string())]);
}

/// Step 1 test 2: resubmitting the same `(project_id, source, idempotency_key)`
/// with the same payload is a `Duplicate` carrying the original event id and
/// its status — and it must not create a second event row or a second job.
#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_resubmission_returns_same_event_without_second_job(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("ING-2")).await.unwrap();
    let payload = json!({"text": "export broken"});
    let first = submit_event(
        &pool,
        submit_req(boot.project_id, boot.human_id, "feishu-msg-2", payload.clone()),
    )
    .await
    .unwrap();
    let SubmitOutcome::Accepted { event_id: first_id } = first else {
        panic!("expected SubmitOutcome::Accepted, got {first:?}")
    };

    let second = submit_event(
        &pool,
        submit_req(boot.project_id, boot.human_id, "feishu-msg-2", payload),
    )
    .await
    .unwrap();
    let SubmitOutcome::Duplicate { event_id, status } = second else {
        panic!("expected SubmitOutcome::Duplicate, got {second:?}")
    };
    assert_eq!(event_id, first_id, "duplicate must point at the original event");
    assert_eq!(status, "received", "duplicate must carry the original event status");

    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM integration.inbound_event), \
                (SELECT count(*) FROM integration.job)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1), "duplicate must not add an event row or a job row");
}

/// Step 1 test 3: same key with a different payload is rejected with
/// `IdempotencyConflict` naming the existing event; nothing new is written.
#[sqlx::test(migrations = "../../migrations")]
async fn same_key_with_different_payload_is_conflict(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("ING-3")).await.unwrap();
    let first = submit_event(
        &pool,
        submit_req(boot.project_id, boot.human_id, "feishu-msg-3", json!({"text": "v1"})),
    )
    .await
    .unwrap();
    let SubmitOutcome::Accepted { event_id: first_id } = first else {
        panic!("expected SubmitOutcome::Accepted, got {first:?}")
    };

    let err = submit_event(
        &pool,
        submit_req(boot.project_id, boot.human_id, "feishu-msg-3", json!({"text": "v2"})),
    )
    .await
    .unwrap_err();
    match err {
        IngestionError::IdempotencyConflict { existing_event_id } => {
            assert_eq!(existing_event_id, first_id);
        }
        other => panic!("expected IngestionError::IdempotencyConflict, got {other:?}"),
    }

    // The conflicting submission must not have mutated or duplicated the event,
    // and the single ai_analysis job is untouched.
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM integration.inbound_event), \
                (SELECT count(*) FROM integration.job)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1));
    let hash: String =
        sqlx::query_scalar("SELECT payload_hash FROM integration.inbound_event WHERE id = $1")
            .bind(first_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(hash, expected_hash(&json!({"text": "v1"})), "original payload must survive");
}

/// Step 1 test 4: a submission whose project does not exist (and one whose
/// actor does not exist) hits the inbound_event FKs; both map to
/// `UnknownProject` — the brief's enum has no actor-specific variant, so the
/// unknown actor is carried by `UnknownProject` too.
#[sqlx::test(migrations = "../../migrations")]
async fn unknown_project_and_actor_map_to_unknown_project(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("ING-4")).await.unwrap();

    let err = submit_event(
        &pool,
        submit_req(Uuid::new_v4(), boot.human_id, "feishu-msg-4a", json!({"text": "x"})),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, IngestionError::UnknownProject), "unknown project, got {err:?}");

    let err = submit_event(
        &pool,
        submit_req(boot.project_id, Uuid::new_v4(), "feishu-msg-4b", json!({"text": "x"})),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, IngestionError::UnknownProject), "unknown actor, got {err:?}");

    // Neither failed submission may leave rows behind.
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM integration.inbound_event), \
                (SELECT count(*) FROM integration.job)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
}
