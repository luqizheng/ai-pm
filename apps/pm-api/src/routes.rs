//! REST routes (Task 9): `POST /api/v1/events` over the idempotent ingestion
//! service ([`pm_application::ingestion::submit_event`]).
//!
//! Response mapping:
//!
//! | outcome                | status | body                                                        |
//! |------------------------|--------|-------------------------------------------------------------|
//! | `Accepted`             | 202    | `{event_id, processing_status: "queued"}`                   |
//! | `Duplicate`            | 200    | `{event_id, status, processing_status: "duplicate"}`        |
//! | `IdempotencyConflict`  | 409    | `{error: "idempotency_conflict", event_id}`                 |
//! | `UnknownProject`       | 404    | `{error: "unknown_project"}`                                |
//! | `Db`                   | 500    | `{error: "internal"}` (details only in the log)             |

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use pm_application::ingestion::{IngestionError, SubmitEventRequest, SubmitOutcome, submit_event};
use serde_json::json;
use sqlx::PgPool;

/// The pm-api route table, stateful over the shared connection pool. Used by
/// `main.rs` and by `tests/routes.rs` so tests exercise the real routes.
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/v1/events", post(submit_event_handler))
        .with_state(pool)
}

async fn submit_event_handler(
    State(pool): State<PgPool>,
    Json(req): Json<SubmitEventRequest>,
) -> Response {
    match submit_event(&pool, req).await {
        Ok(SubmitOutcome::Accepted { event_id }) => (
            StatusCode::ACCEPTED,
            Json(json!({ "event_id": event_id, "processing_status": "queued" })),
        )
            .into_response(),
        Ok(SubmitOutcome::Duplicate { event_id, status }) => (
            StatusCode::OK,
            Json(json!({
                "event_id": event_id,
                "status": status,
                "processing_status": "duplicate",
            })),
        )
            .into_response(),
        Err(IngestionError::IdempotencyConflict { existing_event_id }) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "idempotency_conflict", "event_id": existing_event_id })),
        )
            .into_response(),
        Err(IngestionError::UnknownProject) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "unknown_project" })),
        )
            .into_response(),
        Err(IngestionError::Db(err)) => {
            tracing::error!(error = ?err, "ingestion failed with a database error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
                .into_response()
        }
    }
}
