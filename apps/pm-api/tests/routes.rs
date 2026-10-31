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
use uuid::Uuid;

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

// ------------------------------------------------------- Task 15 supervision

/// Seeds the supervision smoke fixtures: a task at revision 3 plus the audit
/// row of an executed `update_task_status` (with its Restore compensation
/// snapshot) and the executed proposal pointing at it — the minimum rows a
/// veto needs to compensate for real.
async fn seed_executed_proposal(pool: &PgPool, project_id: Uuid, actor_id: Uuid) -> Uuid {
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, title, status, revision) \
         VALUES ($1, $2, 'SUP-TASK revert me', 'completed', 4)",
    )
    .bind(task_id)
    .bind(project_id)
    .execute(pool)
    .await
    .unwrap();

    // The audit row of the executed command, carrying the status-recovery
    // snapshot (restore status 'completed' -> 'running', guarded at revision 4).
    let audit_id = Uuid::new_v4();
    let compensation = serde_json::json!({
        "Restore": [
            {
                "table": "task",
                "target_id": task_id,
                "fields": { "status": "running", "revision": 4 }
            }
        ]
    });
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, action, target_type, target_id, before_data, after_data, compensation_command) \
         VALUES ($1, $2, $3, 'update_task_status', 'task', $4, $5, $6, $7)",
    )
    .bind(audit_id)
    .bind(project_id)
    .bind(actor_id)
    .bind(task_id)
    .bind(serde_json::json!({ "status": "running", "revision": 3 }))
    .bind(serde_json::json!({ "status": "completed", "revision": 4 }))
    .bind(compensation)
    .execute(pool)
    .await
    .unwrap();

    // The proposal chain: event -> analysis_run -> executed proposal.
    let event_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash) \
         VALUES ($1, $2, $3, 'test', 'bug_report', 'sup-seed-1', '{}', 'hash')",
    )
    .bind(event_id)
    .bind(project_id)
    .bind(actor_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai.analysis_run (id, project_id, event_id, model_name, policy_version, config_snapshot) \
         VALUES ($1, $2, $3, 'fake-model', 1, '{}')",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(event_id)
    .execute(pool)
    .await
    .unwrap();

    let proposal_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai.change_proposal \
         (id, project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, \
          proposed_data, rationale, risk_level, status, executed_at, executed_audit_id) \
         VALUES ($1, $2, $3, 0, 'task', $4, 'update_task_status', '{}', 'because', 'low', \
                 'executed', now(), $5)",
    )
    .bind(proposal_id)
    .bind(project_id)
    .bind(run_id)
    .bind(task_id)
    .bind(audit_id)
    .execute(pool)
    .await
    .unwrap();
    proposal_id
}

fn post_json(uri: &str, body: Value) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

/// Veto over HTTP: 200 `rolled_back`, and the compensation really happened —
/// the task is back to `in_progress`, with the intervention and the feed
/// notification on record.
#[sqlx::test(migrations = "../../migrations")]
async fn veto_rolls_back_executed_proposal_over_http(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "SUP-1".to_string(),
            project_name: "supervision smoke".to_string(),
        },
    )
    .await
    .unwrap();
    let proposal_id = seed_executed_proposal(&pool, boot.project_id, boot.ai_principal_id).await;

    let app = router(pool.clone());
    let response = app
        .oneshot(post_json(
            "/api/v1/supervision/veto",
            json!({
                "project_id": boot.project_id,
                "proposal_id": proposal_id,
                "actor_id": boot.human_id,
                "reason": "revert over http",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["outcome"], json!("rolled_back"));
    assert!(body["new_audit_id"].is_string(), "the rollback audit id must be returned");

    // The compensation really restored the task (4 -> 5, status back).
    let (status, revision): (String, i64) = sqlx::query_as(
        "SELECT status, revision FROM delivery.task \
         WHERE project_id = $1 AND title = 'SUP-TASK revert me'",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "running");
    assert_eq!(revision, 5);

    // Intervention and feed notification exist.
    let interventions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.human_intervention \
         WHERE project_id = $1 AND intervention_type = 'veto'",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(interventions, 1);
    let notifications: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.notification \
         WHERE project_id = $1 AND kind = 'veto_applied'",
    )
    .bind(boot.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(notifications, 1);
}

/// Vetoing an unknown proposal answers 404 (and records nothing).
#[sqlx::test(migrations = "../../migrations")]
async fn veto_unknown_proposal_answers_404(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "SUP-2".to_string(),
            project_name: "supervision smoke 2".to_string(),
        },
    )
    .await
    .unwrap();
    let app = router(pool.clone());
    let response = app
        .oneshot(post_json(
            "/api/v1/supervision/veto",
            json!({
                "project_id": boot.project_id,
                "proposal_id": Uuid::new_v4(),
                "actor_id": boot.human_id,
                "reason": "nothing there",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["error"], json!("proposal_not_found"));
}

/// The notification feed over HTTP: 200 JSON array, newest-first, with a
/// working `limit` query parameter.
#[sqlx::test(migrations = "../../migrations")]
async fn notifications_feed_over_http(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "SUP-FEED".to_string(),
            project_name: "feed smoke".to_string(),
        },
    )
    .await
    .unwrap();
    for (idx, kind) in ["older_first", "newer_second"].iter().enumerate() {
        sqlx::query(
            "INSERT INTO integration.notification (project_id, kind, payload, created_at) \
             VALUES ($1, $2, $3, now() - make_interval(mins => $4))",
        )
        .bind(boot.project_id)
        .bind(kind)
        .bind(json!({ "seq": idx }))
        .bind((1 - idx) as i32)
        .execute(&pool)
        .await
        .unwrap();
    }

    let app = router(pool.clone());
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/projects/{}/notifications?limit=10",
                    boot.project_id
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let rows = body.as_array().expect("the feed is a JSON array");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["kind"], json!("newer_second"));
    assert_eq!(rows[1]["kind"], json!("older_first"));
    assert!(rows[0]["id"].is_string());
    assert!(rows[0]["payload"].is_object());
    assert!(rows[0]["created_at"].is_string());
    assert!(rows[0]["veto_window_until"].is_null());

    // limit=1 truncates to the newest row only.
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/projects/{}/notifications?limit=1",
                    boot.project_id
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let rows = body.as_array().expect("the feed is a JSON array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["kind"], json!("newer_second"));
}
            
