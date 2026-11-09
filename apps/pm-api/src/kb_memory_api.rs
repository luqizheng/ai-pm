//! `/v1/kb/*` KB 管理面（KP-3 KB 三层最小面；差异说明 D-KP3-5）。
//!
//! §12.2 K 表只列 K4（审核转达）/ K5（来源只读查询）；最小面另需的入口在此承接，
//! 由 `pm_application::kb_service` 的 KB 应用命令直接落地（V1 同进程网关，**端口形状
//! 不变**）：
//!
//! | 入口 | 用途 |
//! | --- | --- |
//! | `POST /v1/kb/sources`（挂在 K5 同路径，见 `knowledge_services.rs`） | Original Sources 登记（团队池去重 + 项目关联） |
//! | `GET  /v1/kb/sources/{source_id}` | 来源详情（含内联正文） |
//! | `POST /v1/kb/memories` | 新增记忆提案（CREATE → 待审核区） |
//! | `POST /v1/kb/memories/{memory_id}/changes` | 修改 / 撤回提案（UPDATE / WITHDRAW） |
//! | `GET  /v1/kb/memories` | 记忆列表（`?state=` 过滤） |
//! | `GET  /v1/kb/memories/{memory_id}` | 记忆详情 + 全部修订历史（版本还原） |
//!
//! 鉴权与 K 路由同口径：全局 fail-closed 中间件链 + handler 内 `?project_id=` 必填与
//! `require_project_visible`（非成员一律 404）。提案人可以是真人或 Key 调用方（AI
//! 提议）；**审核只走 K4**，且只有真人（AI 无确认权）。引用里的 Platform 对象由
//! `discussion_memory::resolve_refs` 校验项目 / 属主归属（会话消息属主私有）。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use pm_application::discussion_memory::{resolve_refs, MemoryRefRequest};
use pm_application::kb_service::document;
use pm_application::kb_service::memory::{self, ChangeAction, MemoryContent};
use pm_application::kb_service::source::{self, RegisterSourceRequest};
use pm_application::kb_service::KbServiceError;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/kb/sources/{source_id}", get(get_source))
        .route("/v1/kb/memories", get(list_memories).post(propose_memory))
        .route("/v1/kb/memories/{memory_id}", get(get_memory))
        .route(
            "/v1/kb/memories/{memory_id}/changes",
            post(propose_memory_change),
        )
        .route(
            "/v1/kb/documents/{document_id}/revisions/{revision_id}/confirm",
            post(confirm_document_revision),
        )
        .route("/v1/kb/documents", get(list_kb_documents))
        .route("/v1/kb/documents/{document_id}", get(get_kb_document))
}

impl From<KbServiceError> for ApiError {
    fn from(e: KbServiceError) -> Self {
        match e {
            KbServiceError::NotFound(m) => ApiError::NotFound(m),
            KbServiceError::Forbidden(m) => ApiError::Forbidden(m),
            KbServiceError::Invalid(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            KbServiceError::Conflict(m) => ApiError::Text(StatusCode::CONFLICT, m),
            KbServiceError::Db(err) => ApiError::Internal(err),
        }
    }
}

/// `?project_id=` 必填（K 路由同口径）。
fn require_project(query: &HashMap<String, String>) -> ApiResult<Uuid> {
    query
        .get("project_id")
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or(ApiError::BadRequest("project_id is required"))
}

async fn scope(
    state: &AppState,
    caller: &AuthedPrincipal,
    query: &HashMap<String, String>,
) -> ApiResult<Uuid> {
    let project_id = require_project(query)?;
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    Ok(project_id)
}

fn parse_body<T: serde::de::DeserializeOwned>(body: serde_json::Value) -> ApiResult<T> {
    serde_json::from_value(body).map_err(|e| {
        ApiError::Text(
            StatusCode::BAD_REQUEST,
            format!("invalid request body: {e}"),
        )
    })
}

/// 非 UUID 的资源 id 与不存在同口径（不泄露存在性）。
fn path_id(raw: &str, what: &'static str) -> ApiResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| ApiError::NotFound(what))
}

// ── Original Sources ────────────────────────────────────────────────────────

/// `POST /v1/kb/sources`：登记（新建或新关联 201；同项目幂等重放 200）。
pub(crate) async fn register_source(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Response> {
    let project_id = scope(&state, &caller, &query).await?;
    let request: RegisterSourceRequest = parse_body(body)?;
    let receipt = source::register_source(
        &state.pool,
        caller.tenant_id,
        project_id,
        &caller.audit_actor(),
        request,
    )
    .await?;
    let status = if receipt.newly_linked {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(receipt)).into_response())
}

async fn get_source(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(source_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = scope(&state, &caller, &query).await?;
    let source_id = path_id(&source_id, "source not found")?;
    let view = source::get_source(&state.pool, project_id, source_id).await?;
    Ok(Json(json!(view)))
}

// ── Document Revisions（WP-0 内容管线接缝）────────────────────────────────

/// `POST /v1/kb/documents/{document_id}/revisions/{revision_id}/confirm`：
/// 确认发布（draft → published；幂等重确认 200 且不改原确认人/确认时间）。
///
/// **确认权三重门**（WP-0 裁决 Q-A=A\*）：可信人类交互上下文（`auth_mode=interactive`
/// ——AI/Service Key 一律 403）+ 真人主体 + 项目成员。`scope()` 已含
/// `require_project_visible`——非成员 404（不泄漏项目存在性）。
async fn confirm_document_revision(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((document_id, revision_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = scope(&state, &caller, &query).await?;
    let document_id = path_id(&document_id, "kb document revision not found")?;
    let revision_id = path_id(&revision_id, "kb document revision not found")?;
    let outcome = document::confirm_revision(
        &state.pool,
        caller.tenant_id,
        project_id,
        document_id,
        revision_id,
        &caller.audit_actor(),
    )
    .await?;
    Ok(Json(json!(outcome)))
}


// ── KB 文档库读面（前端 KnowledgeView 消费）────────────────────────────────

/// `GET /v1/kb/documents?project_id=`：项目 KB 文档列表（最新修订投影）。
async fn list_kb_documents(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = scope(&state, &caller, &query).await?;
    let items =
        document::list_documents(&state.pool, caller.tenant_id, project_id).await?;
    Ok(Json(json!({ "items": items })))
}

/// `GET /v1/kb/documents/{document_id}?project_id=`：详情（正文 + 修订历史）。
async fn get_kb_document(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(document_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = scope(&state, &caller, &query).await?;
    let document_id = path_id(&document_id, "kb document not found")?;
    let detail =
        document::get_document(&state.pool, caller.tenant_id, project_id, document_id).await?;
    Ok(Json(json!({
        "document": detail.document,
        "content": detail.content,
        "revisions": detail.revisions,
    })))
}

// ── Structured Memory（L1）──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ProposeMemoryBody {
    memory_type: String,
    title: String,
    summary: String,
    #[serde(default)]
    body: Option<String>,
    change_reason: String,
    #[serde(default)]
    refs: Vec<MemoryRefRequest>,
}

#[derive(Debug, Deserialize)]
struct ProposeChangeBody {
    /// `update` | `withdraw`。
    action: String,
    #[serde(default)]
    memory_type: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    body: Option<String>,
    change_reason: String,
    #[serde(default)]
    refs: Vec<MemoryRefRequest>,
}

/// `POST /v1/kb/memories`：新增记忆提案（进待审核区，201）。
async fn propose_memory(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Response> {
    let project_id = scope(&state, &caller, &query).await?;
    let body: ProposeMemoryBody = parse_body(body)?;
    let refs = resolve_refs(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
        body.refs,
    )
    .await?;
    let receipt = memory::propose_create(
        &state.pool,
        caller.tenant_id,
        project_id,
        &caller.audit_actor(),
        MemoryContent {
            memory_type: body.memory_type,
            title: body.title,
            summary: body.summary,
            body: body.body,
        },
        &body.change_reason,
        refs,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(receipt)).into_response())
}

/// `POST /v1/kb/memories/{memory_id}/changes`：修改 / 撤回提案（201）。
async fn propose_memory_change(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(memory_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Response> {
    let project_id = scope(&state, &caller, &query).await?;
    let memory_id = path_id(&memory_id, "memory not found")?;
    let body: ProposeChangeBody = parse_body(body)?;
    let action = ChangeAction::parse(&body.action).ok_or(ApiError::BadRequest(
        "action must be one of update | withdraw",
    ))?;
    let content = match action {
        ChangeAction::Update => match (body.memory_type, body.title, body.summary) {
            (Some(memory_type), Some(title), Some(summary)) => Some(MemoryContent {
                memory_type,
                title,
                summary,
                body: body.body,
            }),
            _ => {
                return Err(ApiError::BadRequest(
                    "update requires memory_type, title and summary",
                ))
            }
        },
        ChangeAction::Withdraw => {
            if body.memory_type.is_some()
                || body.title.is_some()
                || body.summary.is_some()
                || body.body.is_some()
            {
                return Err(ApiError::BadRequest(
                    "withdraw carries no new content (only change_reason)",
                ));
            }
            None
        }
    };
    let refs = resolve_refs(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
        body.refs,
    )
    .await?;
    let receipt = memory::propose_change(
        &state.pool,
        caller.tenant_id,
        project_id,
        memory_id,
        &caller.audit_actor(),
        action,
        content,
        &body.change_reason,
        refs,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(receipt)).into_response())
}

async fn list_memories(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = scope(&state, &caller, &query).await?;
    let items = memory::list_memories(
        &state.pool,
        caller.tenant_id,
        project_id,
        query.get("state").map(String::as_str),
    )
    .await?;
    Ok(Json(json!({ "items": items })))
}

async fn get_memory(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(memory_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let project_id = scope(&state, &caller, &query).await?;
    let memory_id = path_id(&memory_id, "memory not found")?;
    let detail = memory::get_memory(&state.pool, caller.tenant_id, project_id, memory_id).await?;
    Ok(Json(json!(detail)))
}
