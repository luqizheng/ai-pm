//! `/v1/pm/projects/{project_id}/conversations`：最小对话存储 REST（V2 D-4=A）。
//!
//! - `POST   /conversations`                        body `{title?}` → 201 会话视图；
//! - `GET    /conversations`                        本人会话列表（updated_at DESC）；
//! - `POST   /conversations/{id}/messages`          body `{content, content_type?, mode?,
//!   discovery_session_id?}` → 201。**DK-A 唯一发送入口**：服务端判定落点
//!   （`pm_application::conversation_router`，设计 §2.4），响应 = 落点消息视图平铺，
//!   另加 `{landed: "chat"|"interview", discovery_session_id, handoff}`；chat 落点的
//!   消息字段与旧契约一致（role 恒 `user`），interview 落点为 discovery 消息视图；
//! - `POST   /conversations/{id}/handoff/undo`      DK-A 撤销升级（A4，30 分钟窗口）；
//! - `GET    /conversations/{id}/messages?before=&after=&after_id=&limit=&latest=`
//!   `created_at, id` ASC；`before` 为向后 keyset 游标（`created_at < before`），
//!   `after`(+`after_id`) 为向前增量游标（Batch G 前端轮询：
//!   `created_at > after OR (created_at = after AND id > after_id)`）；
//!   before/after 同给 400。Batch H `latest=true`：首屏「最新窗口」——
//!   DESC 取 limit 行再反转为 ASC（与 before/after 同给 400）。
//!
//! 边界：项目不可见（非成员 / 跨租户 / 软删）→ 404；会话不存在或**跨属主**
//! 一律 404（防探测）。发送入口在开关 on 时可能同步调用意图分类器（失败落 chat）。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use pm_application::ai_conversations::{
    create_conversation, list_conversations, list_messages, ContextAnchor, ConversationError,
    ConversationView, MessageView,
};
use pm_application::ai_interactions::{
    submit_interaction, InteractionError, InteractionRequest,
};
use pm_application::conversation_router::{
    send_message, undo_handoff, RouterError, SendMessageRequest, UndoOutcome,
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
    /// Batch 2（0003）：线程级上下文锚点（建会话时定格，消息级不收）。
    /// 缺省 = 无锚点；非法形态由服务层校验后 400。
    pub context: Option<ContextAnchor>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AppendMessageBody {
    pub content: String,
    pub content_type: Option<String>,
    /// DK-A 规则 2：`"interview"` = 用户显式进入需求澄清（新建访谈并升级）。
    pub mode: Option<String>,
    /// DK-A 规则 2：显式续接本项目指定访谈会话。
    pub discovery_session_id: Option<Uuid>,
}

impl From<InteractionError> for ApiError {
    fn from(e: InteractionError) -> Self {
        match e {
            // 不存在与跨属主同形 404（不泄露存在性）。
            InteractionError::NotFound => ApiError::NotFound("interaction_target_not_found"),
            InteractionError::NotInteractive => {
                ApiError::BadRequest("message has no interaction envelope")
            }
            InteractionError::Validation(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            InteractionError::Conflict(code) => ApiError::Conflict(code),
            InteractionError::Db(detail) => ApiError::Internal(sqlx::Error::Protocol(detail)),
        }
    }
}

impl From<RouterError> for ApiError {
    fn from(e: RouterError) -> Self {
        match e {
            RouterError::ConversationNotFound => ApiError::NotFound("conversation_not_found"),
            RouterError::SessionNotFound => ApiError::NotFound("discovery_session_not_found"),
            RouterError::Validation(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            RouterError::Conflict(code) => ApiError::Conflict(code),
            RouterError::Db(detail) => ApiError::Internal(sqlx::Error::Protocol(detail)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListMessagesQuery {
    /// RFC3339 时间戳：只取 `created_at < before`（keyset 游标）。
    pub before: Option<String>,
    /// RFC3339 时间戳：增量游标——取 `created_at > after`（Batch G 前向）。
    pub after: Option<String>,
    /// 同微秒决胜键：`created_at = after AND id > after_id`（须与 after 成对）。
    pub after_id: Option<Uuid>,
    pub limit: Option<i64>,
    /// Batch H 首屏「最新窗口」：true 时按 `(created_at, id)` DESC 取 limit
    /// 行再反转为 ASC 返回（最新 limit 条）；缺省 false（ASC 取最旧 limit
    /// 条）。与 before/after 任一同给 400（从简：组合语义不做）。
    pub latest: Option<bool>,
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
        .route(
            "/v1/pm/projects/{project_id}/conversations/{conversation_id}/handoff/undo",
            post(undo_handoff_handler),
        )
        .route(
            "/v1/pm/projects/{project_id}/conversations/{conversation_id}/messages/{message_id}/interaction",
            post(submit_interaction_handler),
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
        body.context.as_ref(),
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
    let outcome = send_message(
        &state.pool,
        state.conversation_routing.classifier.as_ref(),
        state.conversation_routing.mode,
        principal.tenant_id,
        project_id,
        principal.principal_id,
        conversation_id,
        // REST 恒 user（落点由服务层判定）；metadata 不外收。
        SendMessageRequest {
            content: body.content,
            content_type: body.content_type,
            mode: body.mode,
            discovery_session_id: body.discovery_session_id,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(outcome)).into_response())
}

/// Batch 3C 交互写回：卡片终态落库 + 信封回读（同事务）；终态幂等 / 换
/// action 409 / 属主外 404 同形。
#[derive(Debug, Deserialize)]
pub(crate) struct InteractionBody {
    pub action: String,
    pub reason: Option<String>,
    pub option_id: Option<String>,
    pub answer: Option<serde_json::Value>,
}

async fn submit_interaction_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id, message_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<InteractionBody>,
) -> ApiResult<Response> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let view = submit_interaction(
        &state.pool,
        principal.tenant_id,
        project_id,
        conversation_id,
        message_id,
        principal.principal_id,
        &InteractionRequest {
            action: body.action,
            reason: body.reason,
            option_id: body.option_id,
            answer: body.answer,
        },
    )
    .await?;
    // 幂等重放 200，新建 201。
    let status = if view.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(view)).into_response())
}

/// DK-A 撤销升级（A4）：30 分钟窗口内回退线程链接；本次升级新建的访谈置 PAUSED。
async fn undo_handoff_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthedPrincipal>,
    Path((project_id, conversation_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<UndoOutcome>> {
    require_project_visible(
        &state.pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    let outcome = undo_handoff(
        &state.pool,
        principal.tenant_id,
        project_id,
        principal.principal_id,
        conversation_id,
    )
    .await?;
    Ok(Json(outcome))
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
    let after = match query.after.as_deref() {
        Some(text) => match chrono::DateTime::parse_from_rfc3339(text) {
            Ok(t) => Some(t.with_timezone(&chrono::Utc)),
            Err(_) => {
                return Err(ApiError::BadRequest("after must be RFC3339"));
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
        after,
        query.after_id,
        query.limit,
        // Batch H：缺省 false（旧语义），前端首屏显式传 true。
        query.latest.unwrap_or(false),
    )
    .await?;
    Ok(Json(items))
}
