//! 会话上下文条目端点（AI Collaboration V1.0 / AI-02 · 后端契约 CX3 · B2）。
//!
//! 路由（挂 `/v1/pm/projects/{project_id}/conversations/{conversation_id}/context-items`）：
//! - `GET`    列条目（`?include_detached=true` 供 History；**404 同形**——
//!            跨属主 / 跨项目不泄露存在性）；
//! - `POST`   批量附加（逐项校验存在性 / 权限 / 项目归属 / 版本引用）；
//! - `PATCH`  `/{item_id}/lifecycle` 生命周期流转（**状态机白名单**；
//!            pending → active 需真人主体）；
//! - `DELETE` `/{item_id}` 软移除（detached，保留审计）。
//!
//! 错误映射：ContextItemError::NotFound → 404（同形）· Validation → 400 ·
//! Forbidden → 403 · Conflict → 409 · Db → 500。

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, patch},
    Extension, Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use pm_application::ai_context_items::{
    attach_items, list_candidates, list_interactions, list_items, patch_lifecycle, remove_item,
    AttachItemRequest, ContextItemError, PatchLifecycleRequest,
};

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/conversations/{conversation_id}/context-items",
            get(list_context_items_handler).post(attach_context_items_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/conversations/{conversation_id}/context-items/{item_id}",
            delete(remove_context_item_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/conversations/{conversation_id}/context-items/{item_id}/lifecycle",
            patch(patch_context_item_lifecycle_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/conversations/{conversation_id}/interactions",
            get(list_conversation_interactions_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/context-candidates",
            get(list_context_candidates_handler),
        )
}

fn map_error(err: ContextItemError) -> ApiError {
    match err {
        ContextItemError::NotFound => ApiError::NotFound("context item not found"),
        ContextItemError::Validation(msg) => {
            ApiError::Text(axum::http::StatusCode::BAD_REQUEST, msg)
        }
        ContextItemError::Forbidden(msg) => ApiError::Text(axum::http::StatusCode::FORBIDDEN, msg),
        ContextItemError::Conflict(msg) => ApiError::Text(axum::http::StatusCode::CONFLICT, msg),
        ContextItemError::Db(e) => {
            tracing::error!(error = %e, "context item db error");
            ApiError::Internal(sqlx::Error::Protocol(e))
        }
    }
}

#[derive(Deserialize)]
struct ListQuery {
    include_detached: Option<bool>,
}

async fn list_context_items_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items = list_items(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
        conversation_id,
        query.include_detached.unwrap_or(false),
    )
    .await
    .map_err(map_error)?;
    Ok(Json(serde_json::json!({ "items": items })))
}

#[derive(Deserialize)]
struct AttachBody {
    items: Vec<AttachItemBody>,
}

#[derive(Deserialize)]
struct AttachItemBody {
    object_type: String,
    object_id: Uuid,
    object_code: Option<String>,
    revision_id: Option<Uuid>,
    version: Option<i32>,
    content_hash: Option<String>,
    row_version: Option<i64>,
    /// active / pending（缺省 active）。
    initial_lifecycle: Option<String>,
    #[serde(default)]
    read_only: bool,
    source: String,
}

async fn attach_context_items_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AttachBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let items: Vec<AttachItemRequest> = body
        .items
        .into_iter()
        .map(|item| AttachItemRequest {
            object_type: item.object_type,
            object_id: item.object_id,
            object_code: item.object_code,
            revision_id: item.revision_id,
            version: item.version,
            content_hash: item.content_hash,
            row_version: item.row_version,
            initial_lifecycle: item.initial_lifecycle,
            read_only: item.read_only,
            source: item.source,
        })
        .collect();
    let attached = attach_items(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
        conversation_id,
        &caller.audit_actor(),
        &items,
    )
    .await
    .map_err(map_error)?;
    Ok(Json(serde_json::json!({ "items": attached })))
}

#[derive(Deserialize)]
struct PatchLifecycleBody {
    lifecycle: String,
    unavailable_reason: Option<String>,
}

async fn patch_context_item_lifecycle_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id, item_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<PatchLifecycleBody>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let item = patch_lifecycle(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
        conversation_id,
        item_id,
        &caller.audit_actor(),
        &PatchLifecycleRequest {
            lifecycle: body.lifecycle,
            unavailable_reason: body.unavailable_reason,
        },
    )
    .await
    .map_err(map_error)?;
    Ok(Json(serde_json::json!(item)))
}

async fn remove_context_item_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id, item_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let item = remove_item(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
        conversation_id,
        item_id,
        &caller.audit_actor(),
    )
    .await
    .map_err(map_error)?;
    Ok(Json(serde_json::json!(item)))
}

#[derive(Deserialize)]
struct CandidateQuery {
    /// 候选类型（必选单值：AI-02 选择器按类型分页签请求）。
    r#type: String,
    q: Option<String>,
    page: Option<i64>,
    size: Option<i64>,
    /// 给出时逐项计算「已附加」标识。
    conversation_id: Option<Uuid>,
}

/// GET `/v1/pm/projects/{project_id}/context-candidates`：
/// Add Context 选择器的服务端聚合（CX3 终裁——前端不得分别请求五个业务域
/// 再自行拼接权限；类型筛选 + 关键词 + 分页 + 已附加标识 + 版本标签 + 摘要）。
async fn list_context_candidates_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<CandidateQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let (items, total) = list_candidates(
        &state.pool,
        caller.tenant_id,
        project_id,
        &query.r#type,
        query.q.as_deref(),
        query.page.unwrap_or(1),
        query.size.unwrap_or(20),
        query.conversation_id,
    )
    .await
    .map_err(map_error)?;
    Ok(Json(serde_json::json!({ "items": items, "total": total })))
}

/// GET `/v1/pm/projects/{project_id}/conversations/{conversation_id}/interactions`：
/// 交互/审批记录的**权威**读源（CX3 终裁——Actions / History 不再用客户端推断
/// 充当权威数据）。属主隔离 404 同形。
async fn list_conversation_interactions_handler(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_visible(
        &state.pool,
        project_id,
        caller.tenant_id,
        caller.principal_id,
    )
    .await?;
    let records = list_interactions(
        &state.pool,
        caller.tenant_id,
        project_id,
        caller.principal_id,
        conversation_id,
    )
    .await
    .map_err(map_error)?;
    Ok(Json(serde_json::json!({ "interactions": records })))
}
