//! Read-only knowledge document endpoints (V1 slice).
//!
//! `GET /v1/pm/documents?project_id=` — document list with latest revision.
//! `GET /v1/pm/documents/{id}?project_id=` — Markdown body read from the Git
//! knowledge root, with a hash check against the recorded revision.
//! `GET /v1/pm/documents/search?project_id=&q=` — Markdown tree section search.
//!
//! Writes are intentionally absent from REST: they go through the MCP tool
//! `pm_submit_document` (quality gate / red-line workflow first).

use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use pm_application::knowledge::{self, KnowledgeError, KnowledgeRoot};
use pm_application::knowledge_indexer::{
    DbSectionIndexer, IndexOutcome, KnowledgeIndexer, SearchPage,
};
use pm_application::knowledge_search::KnowledgeSearchService;
use pm_application::project_goal;
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
    /// T-060：offset 分页（缺省 20 / 0；`limit` 夹在 1..=100）。
    /// 响应结构不变（不新增 `total` / `has_more`）。
    pub(crate) limit: Option<i64>,
    pub(crate) offset: Option<i64>,
}

/// `GET /v1/pm/projects/{project_id}/goal`：项目正式 Goal（S01 · I4）。
///
/// Q1（随 Project 创建建立）：读取时惰性确保 Goal 文档存在——仅当
/// `AI_PM_KNOWLEDGE_ROOT` 显式配置时才尝试引导（否则跳过，避免测试/无知识仓
/// 环境产生 Git 副作用）；未建立 → `{"goal": null, "missing": true}`，
/// 缺失是显式信号（I4 §4.4），Agent 不得臆测目标。
pub(crate) async fn goal_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    // 惰性引导（幂等）：显式配置了知识仓根目录才尝试。
    if std::env::var("AI_PM_KNOWLEDGE_ROOT")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        let root = KnowledgeRoot::from_env();
        if let Err(err) = project_goal::ensure_goal_document(
            &state.pool,
            &root,
            project_id,
            principal.principal_id,
        )
        .await
        {
            tracing::warn!(project_id = %project_id, error = %err, "goal bootstrap skipped");
        }
    }
    match project_goal::load_official_goal(&state.pool, project_id).await {
        Ok(Some(goal)) => Json(json!({
            "project_id": project_id,
            "missing": false,
            "goal": goal,
        }))
        .into_response(),
        Ok(None) => Json(json!({
            "project_id": project_id,
            "missing": true,
            "goal": null,
            "doc_key": project_goal::GOAL_DOC_KEY,
        }))
        .into_response(),
        Err(err) => error_response(KnowledgeError::Db(err)),
    }
}

pub(crate) async fn search_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(query): Query<SearchQuery>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        query.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    let service = KnowledgeSearchService::new(state.pool.clone(), root);
    let page = SearchPage::normalized(query.limit, query.offset);
    match service
        .search(query.project_id, principal.tenant_id, &query.q, page)
        .await
    {
        Ok(result) => Json(json!({ "items": result.hits })).into_response(),
        Err(err) => error_response(err),
    }
}

pub(crate) async fn list_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Query(query): Query<ProjectQuery>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        query.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        query.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
    /// T-060：乐观并发基线（当前 published 修订 id）。
    pub(crate) base_revision_id: Option<Uuid>,
    /// T-060：写入幂等键。
    pub(crate) operation_id: String,
    pub(crate) created_by: Uuid,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfirmRedlineBody {
    pub(crate) project_id: Uuid,
    pub(crate) confirmed_by: Uuid,
    /// T-060：写入幂等键。
    pub(crate) operation_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VetoRevisionBody {
    pub(crate) project_id: Uuid,
    pub(crate) vetoed_by: Uuid,
    pub(crate) reason: String,
    /// T-060：写入幂等键。
    pub(crate) operation_id: String,
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
            base_revision_id: body.base_revision_id,
            operation_id: body.operation_id,
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        query.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    match pm_application::knowledge_proposal::confirm_redline_proposal(
        &state.pool,
        &root,
        proposal_id,
        body.project_id,
        body.confirmed_by,
        body.operation_id,
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
        body.operation_id,
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        query.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        query.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
    if let Err(resp) = project_in_tenant(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    // 写索引数据 → 经 Coordinator 取 project 锁（§5.5）。
    let guard = match pm_application::knowledge_write_coordinator::acquire(
        &state.pool,
        body.project_id,
        std::time::Duration::from_millis(
            pm_application::knowledge_write_coordinator::DEFAULT_WRITE_LOCK_TIMEOUT_MS,
        ),
    )
    .await
    {
        Ok(guard) => guard,
        Err(err) => return error_response(err),
    };
    let indexer = DbSectionIndexer::new(state.pool.clone(), root);
    let result = indexer.rebuild_document(body.project_id, revision_id).await;
    guard.release().await;
    match result {
        Ok(outcome) => Json(json!({ "outcome": outcome })).into_response(),
        Err(err) => error_response(err),
    }
}

/// 项目级全量重建（§5.5）：顺序遍历该项目**全部 published 修订**，逐个重建。
///
/// 响应只返回计数，不返回逐条明细。
pub(crate) async fn reindex_project_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Json(body): Json<ReindexBody>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
        return resp;
    }
    let root = KnowledgeRoot::from_env();
    let guard = match pm_application::knowledge_write_coordinator::acquire(
        &state.pool,
        body.project_id,
        std::time::Duration::from_millis(
            pm_application::knowledge_write_coordinator::DEFAULT_WRITE_LOCK_TIMEOUT_MS,
        ),
    )
    .await
    {
        Ok(guard) => guard,
        Err(err) => return error_response(err),
    };
    let revisions: Vec<Uuid> = match sqlx::query_scalar(
        "SELECT r.id FROM ai.document_revision r \
           JOIN ai.project_document d ON d.id = r.document_id \
          WHERE d.project_id = $1 AND d.tenant_id = $2 AND r.status = 'published' \
          ORDER BY d.doc_key, r.revision_no",
    )
    .bind(body.project_id)
    .bind(principal.tenant_id)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            guard.release().await;
            return error_response(KnowledgeError::Db(err));
        }
    };
    let indexer = DbSectionIndexer::new(state.pool.clone(), root);
    let mut reindexed = 0i64;
    let mut stale = 0i64;
    let mut failed = 0i64;
    let mut superseded = 0i64;
    for revision_id in revisions {
        match indexer.rebuild_document(body.project_id, revision_id).await {
            Ok(IndexOutcome::Indexed { .. }) => reindexed += 1,
            Ok(IndexOutcome::Stale) => stale += 1,
            Ok(IndexOutcome::Failed { .. }) => failed += 1,
            Ok(IndexOutcome::Superseded) => superseded += 1,
            Err(_) => failed += 1,
        }
    }
    guard.release().await;
    Json(json!({
        "reindexed": reindexed,
        "stale": stale,
        "failed": failed,
        "superseded": superseded,
    }))
    .into_response()
}

pub(crate) async fn migrate_requirement_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(revision_id): Path<Uuid>,
    Json(body): Json<MigrateRequirementBody>,
) -> Response {
    if let Err(resp) = project_in_tenant(
        &state.pool,
        body.project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await
    {
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
        // DEF-P0-03：人工门 / 权限拒绝 → 403（不是 409）。
        KnowledgeError::Forbidden(_) => (StatusCode::FORBIDDEN, "KNOWLEDGE_FORBIDDEN"),
        KnowledgeError::InvalidKey(_) | KnowledgeError::InvalidType(_) => {
            (StatusCode::BAD_REQUEST, "KNOWLEDGE_INVALID_REQUEST")
        }
        KnowledgeError::Exists(_) => (StatusCode::CONFLICT, "KNOWLEDGE_DOCUMENT_EXISTS"),
        // T-060：project 写锁排队超时（可重试，不是数据冲突）。
        KnowledgeError::ProjectWriteBusy(_) => (StatusCode::CONFLICT, "KNOWLEDGE_PROJECT_BUSY"),
        // T-060：base_revision 与当前 head 不一致（不产生 Revision）。
        KnowledgeError::RevisionConflict(_) => {
            (StatusCode::CONFLICT, "KNOWLEDGE_REVISION_CONFLICT")
        }
        // T-060：权威元数据 / Git 根不可用 → 503（**不是**空结果）。
        KnowledgeError::IndexUnavailable(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "KNOWLEDGE_INDEX_UNAVAILABLE",
        ),
        KnowledgeError::Config(_) | KnowledgeError::Io(_) | KnowledgeError::Git(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "KNOWLEDGE_INTERNAL")
        }
        KnowledgeError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "KNOWLEDGE_INTERNAL"),
    };
    let body = json!({ "code": code, "message": err.to_string() });
    (status, Json(body)).into_response()
}
