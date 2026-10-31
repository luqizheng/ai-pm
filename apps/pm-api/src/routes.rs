//! REST routes: `POST /api/v1/events` (Task 9) over the idempotent ingestion
//! service, plus the Task 15 supervision endpoints over the three human
//! powers (veto / red-line decision / freeze-unfreeze) and the notification
//! feed.
//!
//! `POST /api/v1/events` response mapping:
//!
//! | outcome                | status | body                                                        |
//! |------------------------|--------|-------------------------------------------------------------|
//! | `Accepted`             | 202    | `{event_id, processing_status: "queued"}`                   |
//! | `Duplicate`            | 200    | `{event_id, status, processing_status: "duplicate"}`        |
//! | `IdempotencyConflict`  | 409    | `{error: "idempotency_conflict", event_id}`                 |
//! | `UnknownProject`       | 404    | `{error: "unknown_project"}`                                |
//! | `Db`                   | 500    | `{error: "internal"}` (details only in the log)             |
//!
//! Supervision response mapping (Task 15):
//!
//! | call     | outcome              | status | body                                       |
//! |----------|----------------------|--------|--------------------------------------------|
//! | veto     | `RolledBack`         | 200    | `{outcome: "rolled_back", new_audit_id}`   |
//! | veto     | `AlreadyCompensated` | 200    | `{outcome: "already_compensated", already}`|
//! | veto     | `ProposalClosed`     | 200    | `{outcome: "proposal_closed"}`             |
//! | veto     | `RevisionConflict`   | 409    | `{error: "rollback_failed", ...}`          |
//! | veto     | `NotFound`           | 404    | `{error: "proposal_not_found"}`            |
//! | decision | `Executed`/`Rejected`| 200    | `{outcome: "executed"/"rejected", ...}`    |
//! | decision | `AlreadyDecided`     | 200    | `{outcome: "already_decided", already: true}` |
//! | decision | `NotFound`           | 404    | `{error: "approval_not_found"}`            |
//! | freeze   | ok                   | 200    | `{outcome: "frozen"}`                      |
//! | unfreeze | ok                   | 200    | `{outcome: "unfrozen"}`                    |
//! | freeze   | unknown config       | 404    | `{error: "unknown_project"}`               |
//! | feed     | ok                   | 200    | `[NotificationRow...]` newest-first        |

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use pm_application::supervision::{
    DecideRequest, DecisionOutcome, SupervisionError, VetoOutcome, VetoRequest, decide_approval,
    freeze, notifications_feed, unfreeze, veto,
};
use pm_application::{CommandError, PgCommandExecutor, PolicyEngine};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// Shared handler state: the pool and the process-wide command executor. The
/// executor's red-line set comes from the one canonical policy file — the
/// single source of truth, loaded once per process here.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pool: PgPool,
    pub(crate) executor: Arc<PgCommandExecutor>,
}

/// The pm-api route table, stateful over the shared connection pool and
/// executor. Used by `main.rs` and by `tests/routes.rs` so tests exercise the
/// real routes.
pub fn router(pool: PgPool) -> Router {
    let policy = Arc::new(
        PolicyEngine::from_yaml(include_str!("../../../policies/ai-pm-ai-lead-v2.yaml"))
            .expect("canonical policy yaml must parse"),
    );
    let executor = Arc::new(PgCommandExecutor::new(pool.clone(), policy));
    let state = AppState { pool, executor };
    // The MCP service clones the pool inside its own state factory; it cannot
    // share `state` (different service type), hence a separate handle.
    let mcp_pool = state.pool.clone();
    Router::new()
        .route("/api/v1/events", post(submit_event_handler))
        .route("/api/v1/supervision/veto", post(veto_handler))
        .route(
            "/api/v1/supervision/approvals/{id}/decision",
            post(decide_handler),
        )
        .route("/api/v1/supervision/freeze", post(freeze_handler))
        .route("/api/v1/supervision/unfreeze", post(unfreeze_handler))
        .route(
            "/api/v1/projects/{project_id}/notifications",
            get(notifications_handler),
        )
        // `/v1/pm/*` 业务端点（Slice A，前端经典页契约；无 /api 前缀——vite
        // 代理剥掉 /api 后直达这里）。
        .merge(crate::pm_lists::router())
        .nest_service("/mcp", pm_mcp::PmMcpServer::streamable_http_service(mcp_pool))
        .with_state(state)
}

/// The same route table guarded by the bearer-auth layer (`auth::require_bearer`).
/// Served by `main.rs` under `AI_PM_AUTH_MODE=jwks` (the default, fail-closed);
/// the bare [`router`] stays the `off`-mode/test shape so existing tests keep
/// exercising the unauthenticated contract. The layer wraps everything routed
/// above, including the nested `/mcp` service.
pub fn router_with_auth(pool: PgPool, jwks: Arc<digua_common_jwt::JWKSCache>) -> Router {
    let auth = crate::auth::AuthState { jwks };
    router(pool).layer(axum::middleware::from_fn_with_state(
        auth,
        crate::auth::require_bearer,
    ))
}

// ---------------------------------------------------------------- ingestion

use pm_application::ingestion::{IngestionError, SubmitEventRequest, SubmitOutcome, submit_event};

async fn submit_event_handler(
    State(state): State<AppState>,
    Json(req): Json<SubmitEventRequest>,
) -> Response {
    match submit_event(&state.pool, req).await {
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

// --------------------------------------------------------------- supervision

async fn veto_handler(State(state): State<AppState>, Json(req): Json<VetoRequest>) -> Response {
    match veto(&state.pool, &state.executor, req).await {
        Ok(VetoOutcome::RolledBack { new_audit_id }) => (
            StatusCode::OK,
            Json(json!({ "outcome": "rolled_back", "new_audit_id": new_audit_id })),
        )
            .into_response(),
        // Idempotent repeat: still a success for the caller.
        Ok(VetoOutcome::AlreadyCompensated) => (
            StatusCode::OK,
            Json(json!({ "outcome": "already_compensated", "already": true })),
        )
            .into_response(),
        Ok(VetoOutcome::ProposalClosed) => (
            StatusCode::OK,
            Json(json!({ "outcome": "proposal_closed" })),
        )
            .into_response(),
        // The rollback was refused (the target moved on); the service already
        // recorded intervention('rollback_failed').
        Err(SupervisionError::Command(CommandError::RevisionConflict { expected, actual })) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "rollback_failed",
                "expected_revision": expected,
                "actual_revision": actual,
            })),
        )
            .into_response(),
        // An irreversible proposal (no stored compensation spec) refuses the
        // rollback; the service already recorded intervention('rollback_failed').
        Err(SupervisionError::Command(CommandError::Validation(detail))) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "rollback_failed", "detail": detail })),
        )
            .into_response(),
        Err(SupervisionError::NotFound(detail)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "proposal_not_found", "detail": detail })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "veto failed");
            internal_error()
        }
    }
}

async fn decide_handler(
    State(state): State<AppState>,
    Path(approval_id): Path<Uuid>,
    Json(req): Json<DecideRequest>,
) -> Response {
    // The path names the approval; the body's field (if any) is ignored.
    let req = DecideRequest { approval_id, ..req };
    match decide_approval(&state.pool, &state.executor, req).await {
        Ok(DecisionOutcome::Executed { receipt }) => (
            StatusCode::OK,
            Json(json!({
                "outcome": "executed",
                "audit_id": receipt.audit_id,
                "command_kind": receipt.command_kind,
                "target_id": receipt.target_id,
                "resulting_revision": receipt.resulting_revision,
            })),
        )
            .into_response(),
        Ok(DecisionOutcome::Rejected) => {
            (StatusCode::OK, Json(json!({ "outcome": "rejected" }))).into_response()
        }
        // Idempotent repeat: HTTP 200 with `already: true`.
        Err(SupervisionError::Command(CommandError::AlreadyDecided { .. })) => (
            StatusCode::OK,
            Json(json!({ "outcome": "already_decided", "already": true })),
        )
            .into_response(),
        Err(SupervisionError::NotFound(detail)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "approval_not_found", "detail": detail })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "red-line decision failed");
            internal_error()
        }
    }
}

/// Freeze / unfreeze request body.
#[derive(Debug, Deserialize)]
struct FreezeRequest {
    project_id: Uuid,
    actor_id: Uuid,
    reason: String,
}

async fn freeze_handler(State(state): State<AppState>, Json(req): Json<FreezeRequest>) -> Response {
    // The brake flips the automation mode, records the intervention and the
    // notification, and answers 200 (404 when the project has no AI config).
    match freeze(&state.pool, req.project_id, req.actor_id, &req.reason).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "outcome": "frozen" }))).into_response(),
        Err(SupervisionError::NotFound(detail)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "unknown_project", "detail": detail })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "freeze failed");
            internal_error()
        }
    }
}

async fn unfreeze_handler(
    State(state): State<AppState>,
    Json(req): Json<FreezeRequest>,
) -> Response {
    match unfreeze(&state.pool, req.project_id, req.actor_id, &req.reason).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "outcome": "unfrozen" }))).into_response(),
        Err(SupervisionError::NotFound(detail)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "unknown_project", "detail": detail })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "unfreeze failed");
            internal_error()
        }
    }
}

/// Feed query string: `?since=<RFC3339>&limit=<n>`; both optional.
#[derive(Debug, Deserialize)]
struct FeedQuery {
    since: Option<String>,
    limit: Option<i64>,
}

async fn notifications_handler(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<FeedQuery>,
) -> Response {
    let since = match query.since {
        Some(text) => match chrono::DateTime::parse_from_rfc3339(&text) {
            Ok(t) => Some(t.with_timezone(&chrono::Utc)),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "invalid_since", "detail": "since must be RFC3339" })),
                )
                    .into_response();
            }
        },
        None => None,
    };
    match notifications_feed(&state.pool, project_id, since, query.limit.unwrap_or(50)).await {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(err) => {
            tracing::error!(error = ?err, "notification feed failed");
            internal_error()
        }
    }
}

fn internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "internal" })),
    )
        .into_response()
}
