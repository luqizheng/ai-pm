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
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pm_application::knowledge::{self, KnowledgeError, KnowledgeRoot};
use pm_application::knowledge_index;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::routes::AppState;

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
    Query(query): Query<SearchQuery>,
) -> Response {
    let root = KnowledgeRoot::from_env();
    match knowledge_index::search_documents(&state.pool, &root, query.project_id, &query.q).await {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn list_handler(
    State(state): State<AppState>,
    Query(query): Query<ProjectQuery>,
) -> Response {
    match knowledge::list_documents(&state.pool, query.project_id).await {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn get_handler(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    Query(query): Query<ProjectQuery>,
) -> Response {
    let root = KnowledgeRoot::from_env();
    match knowledge::read_document(&state.pool, &root, query.project_id, document_id).await {
        Ok(document) => Json(document).into_response(),
        Err(err) => error_response(err),
    }
}

fn error_response(err: KnowledgeError) -> Response {
    let (status, code) = match &err {
        KnowledgeError::NotFound => (StatusCode::NOT_FOUND, "KNOWLEDGE_DOCUMENT_NOT_FOUND"),
        KnowledgeError::Stale => (StatusCode::CONFLICT, "KNOWLEDGE_DOCUMENT_STALE"),
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
    (status, Json(Value::from(body))).into_response()
}