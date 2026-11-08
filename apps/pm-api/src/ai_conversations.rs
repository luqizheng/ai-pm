//! `/v1/pm/projects/{project_id}/conversations`：最小对话存储 REST（V2 D-4=A）。
//!
//! - `POST   /conversations`                        body `{title?}` → 201 会话视图；
//! - `GET    /conversations`                        本人会话列表（updated_at DESC）；
//! - `POST   /conversations/{id}/messages`          body `{content, content_type?}`
//!   → 201 消息视图；**role 恒 `user`**（body 不收 role——assistant / system 的
//!   生产者是后续 AI 链批次，不走 REST）；
//! - `GET    /conversations/{id}/messages?before=&limit=`
//!   created_at ASC，`before` 为 RFC3339 keyset 游标（`created_at < before`）。
//!
//! 边界：项目不可见（非成员 / 跨租户 / 软删）→ 404；会话不存在或**跨属主**
//! 一律 404（防探测）。消息只存储，不接任何模型调用。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use pm_application::ai_conversations::{
    append_message, create_conversation, list_conversations, list_messages, ConversationError,
    ConversationView, MessageView,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{require_project_visible, ApiError, ApiResult};

impl From<ConversationError> for ApiError {
    fn from(e: ConversationError) -> Self {
        match e {
            // 不存在与跨属主同形 404（不泄露存在性）。
            ConversationError::NotFound => ApiError::NotFound("conversation_not_found"),
            ConversationError::Validation(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            ConversationError::Db(detail) => ApiError::Internal(sqlx::Error::Protocol(detail)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateConversationBody {
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AppendMessageBody {
    pub content: String,
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListMessagesQuery {
    /// RFC3339 时间戳：只取 `created_at < before`（keyset 游标）。
    pub before: Option<String>,
    pub limit: Option<i64>,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/projects/{project_id}/conversations",
            get(list_conversations_handler).post(create_conversation_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/conversations/{conversation_id}/messages",
            get(list_messages_handler).post(append_message_handler),
        )
}

async fn create_conversation_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateConversationBody>,
) -> ApiResult<Response> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let title = body.title.unwrap_or_default();
    let conversation = create_conversation(
        &state.pool,
        principal.tenant_id,
        project_id,
        principal.principal_id,
        &title,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(conversation)).into_response())
}

async fn list_conversations_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ConversationView>>> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let items = list_conversations(
        &state.pool,
        principal.tenant_id,
        project_id,
        principal.principal_id,
    )
    .await?;
    Ok(Json(items))
}

async fn append_message_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AppendMessageBody>,
) -> ApiResult<Response> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let content_type = body.content_type.unwrap_or_else(|| "markdown".into());
    let message = append_message(
        &state.pool,
        principal.tenant_id,
        project_id,
        principal.principal_id,
        conversation_id,
        // REST 恒 user：assistant / system 属内部 AI 链（后续批次）。
        "user",
        &body.content,
        &content_type,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(message)).into_response())
}

async fn list_messages_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ListMessagesQuery>,
) -> ApiResult<Json<Vec<MessageView>>> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let before = match query.before.as_deref() {
        Some(text) => match chrono::DateTime::parse_from_rfc3339(text) {
            Ok(t) => Some(t.with_timezone(&chrono::Utc)),
            Err(_) => {
                return Err(ApiError::BadRequest("before must be RFC3339"));
            }
        },
        None => None,
    };
    let items = list_messages(
        &state.pool,
        principal.tenant_id,
        project_id,
        principal.principal_id,
        conversation_id,
        before,
        query.limit,
    )
    .await?;
    Ok(Json(items))
}
