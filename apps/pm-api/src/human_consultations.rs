//! `/v1/pm/human-consultations`：Human Consultation 正式决策工作项
//! （PA07-IMP-006，PA-07 V1.0 §10.2 / 审查85）。
//!
//! - POST   body `{project_id, task_id?, title, question, product_scope?,
//!   scope_note?}` → pending；
//! - GET    `?project_id=&status=` 列表；
//! - POST `/{id}/start`、`/{id}/decide`（body `{decision_content}`，四元组的
//!   决定人=调用者、时间=now，DB CHECK 强制完整）、`/{id}/cancel`
//!   （body `{reason}` 必填）、`/{id}/supersede`（body `{title, question}`，
//!   既有决定被改变时追加新行）。
//!
//! 未回应不得视为同意：不存在任何自动转 decided 的路径；挂任务的咨询在
//! 决策未完成期间由 task_waiting('needs_decision') 经 claim 门禁阻塞该任务。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use pm_application::human_consultation::{self, ConsultationError, HumanConsultationRow};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

impl From<ConsultationError> for ApiError {
    fn from(e: ConsultationError) -> Self {
        match e {
            ConsultationError::NotFound => ApiError::NotFound("consultation_not_found"),
            ConsultationError::InvalidTransition(_) => ApiError::Conflict("INVALID_TRANSITION"),
            ConsultationError::IncompleteDecision => {
                ApiError::BadRequest("decision_content is required")
            }
            ConsultationError::MissingReason => ApiError::BadRequest("reason is required"),
            ConsultationError::AlreadyDecided => ApiError::Conflict("ALREADY_DECIDED"),
            ConsultationError::Db(err) => ApiError::Internal(err),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateConsultationBody {
    pub project_id: Uuid,
    #[serde(default)]
    pub task_id: Option<Uuid>,
    pub title: String,
    pub question: String,
    /// 适用 Product 范围（product_id 数组）；多 Product 事项分别建行。
    #[serde(default)]
    pub product_scope: Option<serde_json::Value>,
    #[serde(default)]
    pub scope_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListConsultationQuery {
    pub project_id: Uuid,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DecideBody {
    pub decision_content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CancelBody {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SupersedeBody {
    pub title: String,
    pub question: String,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/human-consultations",
            get(list_handler).post(create_handler),
        )
        .route("/v1/pm/human-consultations/{id}/start", post(start_handler))
        .route("/v1/pm/human-consultations/{id}/decide", post(decide_handler))
        .route("/v1/pm/human-consultations/{id}/cancel", post(cancel_handler))
        .route(
            "/v1/pm/human-consultations/{id}/supersede",
            post(supersede_handler),
        )
}

async fn create_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Json(body): Json<CreateConsultationBody>,
) -> ApiResult<Response> {
    require_project_visible(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let actor = principal.audit_actor();
    let id = human_consultation::create_consultation(
        &state.pool,
        principal.tenant_id,
        body.project_id,
        body.task_id,
        &body.title,
        &body.question,
        body.product_scope.unwrap_or_else(|| serde_json::json!([])),
        body.scope_note.as_deref(),
        principal.principal_id,
        &actor,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response())
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(params): Query<ListConsultationQuery>,
) -> ApiResult<Json<Vec<HumanConsultationRow>>> {
    require_project_visible(
        &state.pool,
        params.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let rows = human_consultation::list_consultations(
        &state.pool,
        principal.tenant_id,
        params.project_id,
        params.status.as_deref(),
    )
    .await?;
    Ok(Json(rows))
}

async fn start_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(id): Path<Uuid>,
) -> ApiResult<Response> {
    let actor = principal.audit_actor();
    human_consultation::start_consultation(&state.pool, principal.tenant_id, id, &actor).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn decide_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(id): Path<Uuid>,
    Json(body): Json<DecideBody>,
) -> ApiResult<Response> {
    let actor = principal.audit_actor();
    // 决定人 = 当前调用主体（API 面不接受代填决定人）。
    human_consultation::decide_consultation(
        &state.pool,
        principal.tenant_id,
        id,
        &body.decision_content,
        principal.principal_id,
        &actor,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn cancel_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(id): Path<Uuid>,
    Json(body): Json<CancelBody>,
) -> ApiResult<Response> {
    let actor = principal.audit_actor();
    human_consultation::cancel_consultation(
        &state.pool,
        principal.tenant_id,
        id,
        &body.reason,
        &actor,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn supersede_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(id): Path<Uuid>,
    Json(body): Json<SupersedeBody>,
) -> ApiResult<Response> {
    let actor = principal.audit_actor();
    let new_id = human_consultation::supersede_consultation(
        &state.pool,
        principal.tenant_id,
        id,
        &body.title,
        &body.question,
        principal.principal_id,
        &actor,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": new_id }))).into_response())
}
