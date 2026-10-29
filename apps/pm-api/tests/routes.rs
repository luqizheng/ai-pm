//! REST route smoke test (Task 9, Step 1): drive the real pm-api router with
//! `tower::ServiceExt::oneshot` — no socket — and assert the HTTP contract of
//! `POST /api/v1/events`: 202 + `{event_id, processing_status: "queued"}` on
//! first submission, 200 + `processing_status: "duplicate"` with the same
//! `event_id` on replay, backed by exactly one event and one job row.
//!
//! Project seed uses `bootstrap_project` (Task 8) exactly like the service
//! tests; the pool the router serves from is the test pool `#[sqlx::test]`
//! hands us.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

fn post_events(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/v1/events")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn post_events_accepts_then_replays_as_duplicate(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "API-1".to_string(),
            project_name: "api project".to_string(),
        },
    )
    .await
    .unwrap();
    let app = router(pool.clone());
    let body = json!({
        "project_id": boot.project_id,
        "actor_id": boot.human_id,
        "source": "feishu",
        "event_type": "bug_report",
        "idempotency_key": "feishu-msg-1",
        "payload": {"text": "login returns 500"},
    });

    // First submission: 202 Accepted with the queued marker.
    let response = app.clone().oneshot(post_events(body.clone())).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let first = body_json(response).await;
    let event_id = first["event_id"].as_str().expect("event_id must be a UUID string");
    assert_eq!(first["processing_status"], json!("queued"));

    // The event and its ai_analysis job really exist (1 event, 1 job).
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM integration.inbound_event), \
                (SELECT count(*) FROM integration.job)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1));

    // Same idempotency key replayed over HTTP: 200 with the duplicate marker
    // and the identical event_id.
    let response = app.oneshot(post_events(body)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let replay = body_json(response).await;
    assert_eq!(replay["event_id"].as_str(), Some(event_id));
    assert_eq!(replay["processing_status"], json!("duplicate"));
    assert_eq!(replay["status"], json!("received"));

    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM integration.inbound_event), \
                (SELECT count(*) FROM integration.job)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1), "replay must not add an event row or a job row");
}
