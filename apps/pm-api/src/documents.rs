//! Read-only knowledge document endpoints (V1 slice).
//!
//! `GET /v1/pm/documents?project_id=` — document list with latest revision.
//! `GET /v1/pm/documents/{id}?project_id=` — Markdown body read from the Git
//! knowledge root, with a hash check against the recorded revision.
//! `GET /v1/pm/documents/search?project_id=&q=` — Markdown tree section search.
//!
//! Writes are intentionally absent from REST: they go through the MCP tool
//! `pm_submit_document` (quality gate / red-line workflow first).

use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pm_application::knowledge::{self, KnowledgeError, KnowledgeRoot};
use pm_application::knowledge_index;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;

/// 租户 + 成员边界（对外入口，T-022）：跨租户或调用者不是项目成员的
/// project 一律按不存在处理，不泄露存在性。
async fn project_in_tenant(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> Result<(), Response> {
    match crate::support::project_visible(pool, project_id, tenant_id, principal_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "code": "KNOWLEDGE_PROJECT_NOT_FOUND",
                "message": "project not found",
            })),
        )
            .into_response()),
        Err(err) => Err(error_response(KnowledgeError::Db(err))),
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectQuery {
    pub(crate) project_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchQuery {
    pub(crate) project_id: Uuid,
    pub(crate) q: String,
}

pub(crate) async fn search_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(query): Query<SearchQuery>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, query.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    match knowledge_index::search_documents(&state.pool, &root, query.project_id, &query.q).await {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(query): Query<ProjectQuery>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, query.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    match knowledge::list_documents(&state.pool, query.project_id).await {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn get_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(document_id): Path<Uuid>,
    Query(query): Query<ProjectQuery>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, query.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    match knowledge::read_document(&state.pool, &root, query.project_id, document_id).await {
        Ok(document) => Json(document).into_response(),
        Err(err) => error_response(err),
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProposalQuery {
    pub(crate) project_id: Uuid,
    pub(crate) document_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProposalBody {
    pub(crate) project_id: Uuid,
    pub(crate) doc_key: String,
    pub(crate) doc_type: String,
    pub(crate) markdown: String,
    pub(crate) change_summary: Option<String>,
    pub(crate) source_event_id: Option<Uuid>,
    pub(crate) analysis_run_id: Option<Uuid>,
    pub(crate) source_ref: Option<String>,
    pub(crate) redline_kind: Option<String>,
    pub(crate) base_content_hash: Option<String>,
    pub(crate) created_by: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfirmRedlineBody {
    pub(crate) project_id: Uuid,
    pub(crate) confirmed_by: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VetoRevisionBody {
    pub(crate) project_id: Uuid,
    pub(crate) vetoed_by: Uuid,
    pub(crate) reason: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddReferenceBody {
    pub(crate) project_id: Uuid,
    pub(crate) document_revision_id: Uuid,
    pub(crate) section_anchor: Option<String>,
    pub(crate) section_title: Option<String>,
    pub(crate) start_line: Option<i32>,
    pub(crate) reference_kind: Option<String>,
    pub(crate) source_kind: String,
    pub(crate) source_id: Option<Uuid>,
    pub(crate) created_by: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReindexBody {
    pub(crate) project_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MigrateRequirementBody {
    pub(crate) project_id: Uuid,
    pub(crate) created_by: Uuid,
}

pub(crate) async fn submit_proposal_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Json(body): Json<ProposalBody>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, body.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    let outcome = pm_application::knowledge_proposal::submit_proposal(
        &state.pool,
        &root,
        pm_application::knowledge_proposal::ProposalRequest {
            project_id: body.project_id,
            doc_key: body.doc_key,
            doc_type: body.doc_type,
            markdown: body.markdown,
            change_summary: body.change_summary,
            source_event_id: body.source_event_id,
            analysis_run_id: body.analysis_run_id,
            source_ref: body.source_ref,
            redline_kind: body.redline_kind,
            base_content_hash: body.base_content_hash,
            created_by: body.created_by,
        },
    )
    .await
    .map_err(error_response);
    match outcome {
        Ok(outcome) => Json(outcome).into_response(),
        Err(response) => response,
    }
}

pub(crate) async fn list_proposals_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(query): Query<ProposalQuery>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, query.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    match pm_application::knowledge_proposal::list_proposals(
        &state.pool,
        query.project_id,
        query.document_id,
    )
    .await
    {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn confirm_redline_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(proposal_id): Path<Uuid>,
    Json(body): Json<ConfirmRedlineBody>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, body.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    match pm_application::knowledge_proposal::confirm_redline_proposal(
        &state.pool,
        &root,
        proposal_id,
        body.project_id,
        body.confirmed_by,
    )
    .await
    {
        Ok(outcome) => Json(outcome).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn veto_revision_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(revision_id): Path<Uuid>,
    Json(body): Json<VetoRevisionBody>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, body.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    match pm_application::knowledge_proposal::veto_revision(
        &state.pool,
        &root,
        body.project_id,
        revision_id,
        body.vetoed_by,
        &body.reason,
    )
    .await
    {
        Ok(outcome) => Json(outcome).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn list_references_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(document_id): Path<Uuid>,
    Query(query): Query<ProjectQuery>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, query.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    match pm_application::knowledge_reference::list_references(
        &state.pool,
        query.project_id,
        document_id,
    )
    .await
    {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn add_reference_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(document_id): Path<Uuid>,
    Json(body): Json<AddReferenceBody>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, body.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let id = pm_application::knowledge_reference::add_reference(
        &state.pool,
        pm_application::knowledge_reference::AddReferenceRequest {
            project_id: body.project_id,
            document_id,
            document_revision_id: body.document_revision_id,
            section_anchor: body.section_anchor,
            section_title: body.section_title,
            start_line: body.start_line,
            reference_kind: body
                .reference_kind
                .unwrap_or_else(|| "evidence".to_string()),
            source_kind: body.source_kind,
            source_id: body.source_id,
            created_by: body.created_by,
        },
    )
    .await
    .map_err(error_response);
    match id {
        Ok(id) => Json(json!({ "id": id })).into_response(),
        Err(response) => response,
    }
}

pub(crate) async fn index_state_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(document_id): Path<Uuid>,
    Query(query): Query<ProjectQuery>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, query.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    match pm_application::knowledge_indexer::index_states(
        &state.pool,
        query.project_id,
        document_id,
    )
    .await
    {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn reindex_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(revision_id): Path<Uuid>,
    Json(body): Json<ReindexBody>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, body.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    let indexer =
        pm_application::knowledge_indexer::MarkdownTreeIndexer::new(state.pool.clone(), root);
    match pm_application::knowledge_indexer::index_revision(
        &state.pool,
        &indexer,
        body.project_id,
        revision_id,
    )
    .await
    {
        Ok(()) => Json(json!({ "outcome": "indexed" })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn migrate_requirement_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(revision_id): Path<Uuid>,
    Json(body): Json<MigrateRequirementBody>,
) -> Response {
    if let Err(resp) = project_in_tenant(&state.pool, body.project_id, principal.tenant_id, principal.principal_id).await {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    match pm_application::knowledge_migrate::migrate_requirement_revision(
        &state.pool,
        &root,
        body.project_id,
        revision_id,
        body.created_by,
    )
    .await
    {
        Ok(outcome) => Json(outcome).into_response(),
        Err(err) => error_response(err),
    }
}

fn error_response(err: KnowledgeError) -> Response {
    let (status, code) = match &err {
        KnowledgeError::NotFound => (StatusCode::NOT_FOUND, "KNOWLEDGE_DOCUMENT_NOT_FOUND"),
        KnowledgeError::Stale => (StatusCode::CONFLICT, "KNOWLEDGE_DOCUMENT_STALE"),
        KnowledgeError::Conflict(_) => (StatusCode::CONFLICT, "KNOWLEDGE_CONFLICT"),
        KnowledgeError::InvalidKey(_) | KnowledgeError::InvalidType(_) => {
            (StatusCode::BAD_REQUEST, "KNOWLEDGE_INVALID_REQUEST")
        }
        KnowledgeError::Exists(_) => (StatusCode::CONFLICT, "KNOWLEDGE_DOCUMENT_EXISTS"),
        KnowledgeError::Config(_) | KnowledgeError::Io(_) | KnowledgeError::Git(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "KNOWLEDGE_INTERNAL")
        }
        KnowledgeError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "KNOWLEDGE_INTERNAL"),
    };
    let body = json!({ "code": code, "message": err.to_string() });
    (status, Json(body)).into_response()
}