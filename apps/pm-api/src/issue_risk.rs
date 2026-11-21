//! `/v1/pm/projects/{project_id}/{issues|risks|blockers}`：Issue / Risk /
//! Blocker 领域对象（PA-05-01 收尾，2026-11-21 V1 最小集）。
//!
//! 守卫同 human_consultations 口径：项目可见性（require_project_visible，
//! 不可见 404 不泄露存在性）即读写边界；终态完整性由 DB CHECK 强制。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use pm_application::actor::AuditActor;
use pm_application::issue_risk::{self, WorkDomainError};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

impl From<WorkDomainError> for ApiError {
    fn from(e: WorkDomainError) -> Self {
        match e {
            WorkDomainError::NotFound => ApiError::NotFound("not_found"),
            WorkDomainError::Validation(m) => ApiError::BadRequest(m),
            WorkDomainError::Db(err) => ApiError::Internal(err),
        }
    }
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/issues",
            get(list_issues_handler).post(create_issue_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/risks",
            get(list_risks_handler).post(create_risk_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/blockers",
            get(list_blockers_handler).post(create_blocker_handler),
        )
        .route("/v1/pm/issues/{id}/resolve", post(resolve_issue_handler))
        .route("/v1/pm/issues/{id}/cancel", post(cancel_issue_handler))
        .route(
            "/v1/pm/risks/{id}/mitigate",
            post(|state, principal, path| async move {
                risk_transition_handler(state, principal, path, "mitigating").await
            }),
        )
        .route(
            "/v1/pm/risks/{id}/close",
            post(|state, principal, path| async move {
                risk_transition_handler(state, principal, path, "closed").await
            }),
        )
        .route("/v1/pm/blockers/{id}/clear", post(clear_blocker_handler))
}

fn actor_of(principal: &AuthedPrincipal) -> AuditActor {
    AuditActor {
        principal_id: principal.principal_id,
        identity_id: None,
        client: None,
        auth_mode: None,
        claimed_device_id: None,
    }
}

#[derive(Deserialize)]
struct TitleBody {
    title: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct IssueBody {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    severity: Option<String>,
}

#[derive(Deserialize)]
struct RiskBody {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    impact: Option<String>,
    #[serde(default)]
    probability: Option<String>,
}

#[derive(Deserialize)]
struct BlockerBody {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    blocked_task_id: Option<Uuid>,
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize)]
struct CancelBody {
    reason: String,
}

async fn create_issue_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<IssueBody>,
) -> ApiResult<Response> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let id = issue_risk::create_issue(
        &state.pool,
        principal.tenant_id,
        project_id,
        &body.title,
        body.description.as_deref(),
        body.severity.as_deref(),
        &actor_of(&principal),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response())
}

async fn list_issues_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let items = issue_risk::list_issues(
        &state.pool,
        principal.tenant_id,
        project_id,
        query.status.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn resolve_issue_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(issue_id): Path<Uuid>,
) -> ApiResult<Response> {
    issue_risk::transition_issue(
        &state.pool,
        principal.tenant_id,
        issue_id,
        "resolved",
        None,
        &actor_of(&principal),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn cancel_issue_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(issue_id): Path<Uuid>,
    Json(body): Json<CancelBody>,
) -> ApiResult<Response> {
    issue_risk::transition_issue(
        &state.pool,
        principal.tenant_id,
        issue_id,
        "cancelled",
        Some(&body.reason),
        &actor_of(&principal),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn create_risk_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<RiskBody>,
) -> ApiResult<Response> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let id = issue_risk::create_risk(
        &state.pool,
        principal.tenant_id,
        project_id,
        &body.title,
        body.description.as_deref(),
        body.impact.as_deref(),
        body.probability.as_deref(),
        &actor_of(&principal),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response())
}

async fn list_risks_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let items = issue_risk::list_risks(
        &state.pool,
        principal.tenant_id,
        project_id,
        query.status.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn risk_transition_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(risk_id): Path<Uuid>,
    to: &'static str,
) -> ApiResult<Response> {
    issue_risk::transition_risk(
        &state.pool,
        principal.tenant_id,
        risk_id,
        to,
        &actor_of(&principal),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn create_blocker_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<BlockerBody>,
) -> ApiResult<Response> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let id = issue_risk::create_blocker(
        &state.pool,
        principal.tenant_id,
        project_id,
        &body.title,
        body.description.as_deref(),
        body.blocked_task_id,
        &actor_of(&principal),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response())
}

async fn list_blockers_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let items = issue_risk::list_blockers(
        &state.pool,
        principal.tenant_id,
        project_id,
        query.status.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

async fn clear_blocker_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(blocker_id): Path<Uuid>,
) -> ApiResult<Response> {
    issue_risk::clear_blocker(
        &state.pool,
        principal.tenant_id,
        blocker_id,
        &actor_of(&principal),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
