//! `/v1/pm/projects/{id}/discovery/sessions`：项目认知访谈（T-084）。
//!
//! Web 入口是同一 `DiscoverySession` 的一个 producer（`source=WEB`）；本模块
//! 只暴露访谈需要的读写与发现卡片确认，不做自由聊天、不做 AI tool-loop。

use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use pm_application::discovery::{
    DiscoveryError, FindingCreator, NewFinding, add_finding, add_question, defer_question,
    answer_question, get_session_detail, list_sessions, record_finding_verification,
    start_session_from_import, transition_session,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/discovery/sessions",
            get(list_handler).post(start_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}",
            get(get_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}/status",
            post(status_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}/findings",
            post(add_finding_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}/findings/{finding_id}/verification",
            post(verification_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}/questions",
            post(add_question_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}/questions/{question_id}/answers",
            post(answer_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/discovery/sessions/{session_id}/questions/{question_id}/defer",
            post(defer_handler),
        )
}

fn discovery_error(err: DiscoveryError) -> Response {
    let (status, body) = match err {
        DiscoveryError::ProjectNotFound => (
            StatusCode::NOT_FOUND,
            json!({ "error": "project_not_found" }),
        ),
        DiscoveryError::SessionNotFound => (
            StatusCode::NOT_FOUND,
            json!({ "error": "session_not_found" }),
        ),
        DiscoveryError::FindingNotFound => (
            StatusCode::NOT_FOUND,
            json!({ "error": "finding_not_found" }),
        ),
        DiscoveryError::QuestionNotFound => (
            StatusCode::NOT_FOUND,
            json!({ "error": "question_not_found" }),
        ),
        DiscoveryError::ProposalNotFound => (
            StatusCode::NOT_FOUND,
            json!({ "error": "proposal_not_found" }),
        ),
        DiscoveryError::ImportNotFound => (
            StatusCode::NOT_FOUND,
            json!({ "error": "import_not_found" }),
        ),
        DiscoveryError::InvalidTransition { from, to } => (
            StatusCode::CONFLICT,
            json!({ "error": "invalid_state_transition", "from": from, "to": to }),
        ),
        DiscoveryError::Validation(detail) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "validation_error", "detail": detail }),
        ),
        DiscoveryError::Db(err) => {
            tracing::error!(error = %err, "discovery endpoint database error");
            (StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": "internal" }))
        }
    };
    (status, Json(body)).into_response()
}

/// 租户 + 成员边界（对外入口，T-022）：跨租户或非项目成员一律按不存在处理。
async fn project_in_tenant(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> Result<(), Response> {
    match crate::support::project_visible(pool, project_id, tenant_id, principal_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(discovery_error(DiscoveryError::ProjectNotFound)),
        Err(err) => Err(discovery_error(DiscoveryError::Db(err))),
    }
}

/// question 必须挂在路径指定的 session 下（防止同租户内跨 session 误操作）。
async fn question_in_session(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    question_id: Uuid,
) -> Result<(), Response> {
    let ok: Option<Uuid> = match sqlx::query_scalar(
        "SELECT id FROM discovery.discovery_question \
         WHERE tenant_id = $1 AND id = $2 AND session_id = $3",
    )
    .bind(tenant_id)
    .bind(question_id)
    .bind(session_id)
    .fetch_optional(pool)
    .await
    {
        Ok(ok) => ok,
        Err(err) => return Err(discovery_error(DiscoveryError::Db(err))),
    };
    match ok {
        Some(_) => Ok(()),
        None => Err(discovery_error(DiscoveryError::QuestionNotFound)),
    }
}

#[derive(Deserialize)]
struct StartSessionBody {
    import_id: Option<Uuid>,
}

async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    match list_sessions(&state.pool, principal.tenant_id, project_id).await {
        Ok(items) => {
            let total = items.len();
            (StatusCode::OK, Json(json!({ "items": items, "total": total }))).into_response()
        }
        Err(err) => discovery_error(err),
    }
}

async fn start_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    body: Option<Json<StartSessionBody>>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    let import_id = body.and_then(|Json(b)| b.import_id);
    match start_session_from_import(
        &state.pool,
        principal.tenant_id,
        project_id,
        principal.principal_id,
        import_id,
    )
    .await
    {
        Ok(outcome) => (
            StatusCode::CREATED,
            Json(json!({
                "session": outcome.session,
                "questions_added": outcome.questions_added,
                "linked_import_id": outcome.linked_import_id,
            })),
        )
            .into_response(),
        Err(err) => discovery_error(err),
    }
}

async fn get_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    match get_session_detail(&state.pool, principal.tenant_id, session_id).await {
        Ok(detail) => (StatusCode::OK, Json(detail)).into_response(),
        Err(err) => discovery_error(err),
    }
}

#[derive(Deserialize)]
struct StatusBody {
    status: String,
}

async fn status_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<StatusBody>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    match transition_session(&state.pool, principal.tenant_id, session_id, &body.status).await {
        Ok(session) => (StatusCode::OK, Json(session)).into_response(),
        Err(err) => discovery_error(err),
    }
}

#[derive(Deserialize)]
struct AddFindingBody {
    kind: String,
    content: String,
    confidence: Option<f64>,
    #[serde(default)]
    evidence: serde_json::Value,
    #[serde(default)]
    repository_ids: Vec<Uuid>,
    actor_role: Option<String>,
}

async fn add_finding_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AddFindingBody>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    match add_finding(
        &state.pool,
        principal.tenant_id,
        session_id,
        NewFinding {
            kind: body.kind,
            content: body.content,
            confidence: body.confidence,
            evidence: body.evidence,
            repository_ids: body.repository_ids,
            actor_role: body.actor_role,
        },
        FindingCreator::HumanDirect,
    )
    .await
    {
        Ok(finding) => (StatusCode::CREATED, Json(finding)).into_response(),
        Err(err) => discovery_error(err),
    }
}

#[derive(Deserialize)]
struct VerificationBody {
    status: String,
    answer: String,
    #[serde(default)]
    confirmed_finding: Option<String>,
    #[serde(default)]
    operation_id: Option<String>,
}

async fn verification_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, session_id, finding_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<VerificationBody>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    match record_finding_verification(
        &state.pool,
        principal.tenant_id,
        session_id,
        finding_id,
        &body.status,
        &body.answer,
        principal.principal_id,
        body.confirmed_finding.as_deref(),
        body.operation_id.as_deref(),
    )
    .await
    {
        Ok(outcome) => (StatusCode::OK, Json(outcome)).into_response(),
        Err(err) => discovery_error(err),
    }
}

#[derive(Deserialize)]
struct AddQuestionBody {
    question: String,
    #[serde(default)]
    why_blocking: Option<String>,
    #[serde(default)]
    finding_id: Option<Uuid>,
}

async fn add_question_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AddQuestionBody>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    match add_question(
        &state.pool,
        principal.tenant_id,
        session_id,
        &body.question,
        body.why_blocking.as_deref(),
        body.finding_id,
    )
    .await
    {
        Ok(question) => (StatusCode::CREATED, Json(question)).into_response(),
        Err(err) => discovery_error(err),
    }
}

#[derive(Deserialize)]
struct AnswerBody {
    answer: String,
}

async fn answer_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, session_id, question_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<AnswerBody>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    if let Err(resp) = question_in_session(&state.pool, principal.tenant_id, session_id, question_id).await {
        return resp;
    }
    match answer_question(
        &state.pool,
        principal.tenant_id,
        question_id,
        &body.answer,
        principal.principal_id,
    )
    .await
    {
        Ok(answer) => (StatusCode::CREATED, Json(answer)).into_response(),
        Err(err) => discovery_error(err),
    }
}

async fn defer_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, session_id, question_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Response {
    if let Err(resp) =
        project_in_tenant(&state.pool, project_id, principal.tenant_id, principal.principal_id).await
    {
        return resp;
    }
    if let Err(resp) = question_in_session(&state.pool, principal.tenant_id, session_id, question_id).await {
        return resp;
    }
    match defer_question(&state.pool, principal.tenant_id, question_id).await {
        Ok(question) => (StatusCode::OK, Json(question)).into_response(),
        Err(err) => discovery_error(err),
    }
}