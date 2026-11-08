//! Platform 内最小对话存储（V2 D-4=A，裁决 2026-11-08）。
//!
//! 边界（裁决 §〇）：
//!
//! - **最小对话记忆**——承载 AI PM 对话记忆，**不是** Work Session（稿 §8 挂账）；
//!   结构化交互（task_interaction 链）不动。
//! - 消息只存储：assistant / system 消息的生产者是后续 AI 链批次；本模块对
//!   role 只做词表校验（`user` / `assistant` / `system`），REST 侧只允许 `user`。
//! - 无 outbox / 无审计：对话行不进事件链，也不写治理审计。
//! - 属主隔离：会话按 `principal_id` 隔离，跨属主访问一律
//!   [`ConversationError::NotFound`]（防探测）。

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

/// 消息角色词表（`ai.conversation_message.role` CHECK 同源）。
pub const MESSAGE_ROLES: [&str; 3] = ["user", "assistant", "system"];

/// 默认消息页大小（`list_messages` 未传 limit 时）。
pub const DEFAULT_MESSAGE_LIMIT: i64 = 200;
/// 消息单页上限。
pub const MAX_MESSAGE_LIMIT: i64 = 500;
/// 会话列表上限（updated_at DESC 截断）。
pub const CONVERSATION_LIST_LIMIT: i64 = 100;
/// 标题长度上限。
pub const MAX_TITLE_CHARS: usize = 500;
/// 单条消息内容长度上限。
pub const MAX_CONTENT_CHARS: usize = 100_000;

/// 会话视图（REST 直接序列化）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConversationView {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub principal_id: Uuid,
    pub title: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_message_at: Option<DateTime<Utc>>,
}

/// 消息视图（REST 直接序列化）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MessageView {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub content_type: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// 对话域失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConversationError {
    /// 会话不存在 / 跨属主 / 跨项目（**上层映射 404**，不泄露存在性）。
    #[error("conversation not found")]
    NotFound,
    /// 校验失败。
    #[error("{0}")]
    Validation(String),
    /// 数据库错误。
    #[error("database error: {0}")]
    Db(String),
}

/// 建会话（属主 = 调用 principal；标题缺省空串）。
pub async fn create_conversation(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    title: &str,
) -> Result<ConversationView, ConversationError> {
    let title = title.trim();
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(ConversationError::Validation(format!(
            "title must be at most {MAX_TITLE_CHARS} characters"
        )));
    }
    let row = sqlx::query(
        "INSERT INTO ai.conversation (tenant_id, project_id, principal_id, title) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .map_err(db)?;
    let id: Uuid = row.get("id");
    fetch_conversation(pool, tenant_id, project_id, principal_id, id)
        .await?
        .ok_or(ConversationError::NotFound)
}

/// 列出**本人**在本项目的会话（updated_at DESC，截断 [`CONVERSATION_LIST_LIMIT`]）。
pub async fn list_conversations(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
) -> Result<Vec<ConversationView>, ConversationError> {
    let rows = sqlx::query(
        "SELECT id, tenant_id, project_id, principal_id, title, status, \
                created_at, updated_at, last_message_at \
         FROM ai.conversation \
         WHERE tenant_id = $1 AND project_id = $2 AND principal_id = $3 \
         ORDER BY updated_at DESC \
         LIMIT $4",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .bind(CONVERSATION_LIST_LIMIT)
    .fetch_all(pool)
    .await
    .map_err(db)?;
    Ok(rows.into_iter().map(conversation_from_row).collect())
}

/// 追加一条消息（同事务刷新会话 `updated_at` / `last_message_at`）。
///
/// role 词表见 [`MESSAGE_ROLES`]；REST 侧只允许 `user`（assistant / system
/// 留给内部 AI 链）。会话不存在 / 跨属主 / 跨项目 → [`ConversationError::NotFound`]。
pub async fn append_message(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    role: &str,
    content: &str,
    content_type: &str,
) -> Result<MessageView, ConversationError> {
    if !MESSAGE_ROLES.contains(&role) {
        return Err(ConversationError::Validation(format!(
            "role must be one of {}",
            MESSAGE_ROLES.join("/")
        )));
    }
    let content_type = content_type.trim();
    if content_type.is_empty() {
        return Err(ConversationError::Validation(
            "content_type must not be empty".into(),
        ));
    }
    if content.trim().is_empty() {
        return Err(ConversationError::Validation(
            "content must not be empty".into(),
        ));
    }
    if content.chars().count() > MAX_CONTENT_CHARS {
        return Err(ConversationError::Validation(format!(
            "content must be at most {MAX_CONTENT_CHARS} characters"
        )));
    }

    let mut tx = pool.begin().await.map_err(db)?;
    // 属主锁：同一行串行化，跨属主 / 跨项目 / 不存在统一 NotFound（防探测）。
    let owned = sqlx::query(
        "SELECT id FROM ai.conversation \
         WHERE id = $1 AND tenant_id = $2 AND project_id = $3 AND principal_id = $4 \
         FOR UPDATE",
    )
    .bind(conversation_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db)?;
    if owned.is_none() {
        return Err(ConversationError::NotFound);
    }

    // now() 在 Postgres 里是事务时间戳：消息 created_at 与会话刷新值同源。
    let row = sqlx::query(
        "INSERT INTO ai.conversation_message \
             (conversation_id, role, content, content_type) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, conversation_id, role, content, content_type, metadata, created_at",
    )
    .bind(conversation_id)
    .bind(role)
    .bind(content)
    .bind(content_type)
    .fetch_one(&mut *tx)
    .await
    .map_err(db)?;
    sqlx::query(
        "UPDATE ai.conversation \
         SET updated_at = now(), last_message_at = now() \
         WHERE id = $1",
    )
    .bind(conversation_id)
    .execute(&mut *tx)
    .await
    .map_err(db)?;
    tx.commit().await.map_err(db)?;
    Ok(message_from_row(&row))
}

/// 列出会话内消息（created_at ASC；`before` 为 keyset 游标——只取
/// `created_at < before`；`limit` 缺省 [`DEFAULT_MESSAGE_LIMIT`]，
/// 截到 [`MAX_MESSAGE_LIMIT`]）。
pub async fn list_messages(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    before: Option<DateTime<Utc>>,
    limit: Option<i64>,
) -> Result<Vec<MessageView>, ConversationError> {
    let limit = limit
        .unwrap_or(DEFAULT_MESSAGE_LIMIT)
        .clamp(1, MAX_MESSAGE_LIMIT);
    // 属主 / 项目边界：会话不可见 → NotFound（与 append 同一口径）。
    let owned = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM ai.conversation \
         WHERE id = $1 AND tenant_id = $2 AND project_id = $3 AND principal_id = $4",
    )
    .bind(conversation_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await
    .map_err(db)?;
    if owned.is_none() {
        return Err(ConversationError::NotFound);
    }

    let rows = sqlx::query(
        "SELECT id, conversation_id, role, content, content_type, metadata, created_at \
         FROM ai.conversation_message \
         WHERE conversation_id = $1 AND ($2::timestamptz IS NULL OR created_at < $2) \
         ORDER BY created_at ASC \
         LIMIT $3",
    )
    .bind(conversation_id)
    .bind(before)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(db)?;
    Ok(rows.iter().map(message_from_row).collect())
}

/// 取一行会话（带属主边界）。
async fn fetch_conversation(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
) -> Result<Option<ConversationView>, ConversationError> {
    let rows = sqlx::query(
        "SELECT id, tenant_id, project_id, principal_id, title, status, \
                created_at, updated_at, last_message_at \
         FROM ai.conversation \
         WHERE id = $1 AND tenant_id = $2 AND project_id = $3 AND principal_id = $4",
    )
    .bind(conversation_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .fetch_all(pool)
    .await
    .map_err(db)?;
    Ok(rows.into_iter().next().map(conversation_from_row))
}

fn conversation_from_row(row: sqlx::postgres::PgRow) -> ConversationView {
    ConversationView {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        project_id: row.get("project_id"),
        principal_id: row.get("principal_id"),
        title: row.get("title"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        last_message_at: row.get("last_message_at"),
    }
}

fn message_from_row(row: &sqlx::postgres::PgRow) -> MessageView {
    MessageView {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        role: row.get("role"),
        content: row.get("content"),
        content_type: row.get("content_type"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    }
}

fn db(e: sqlx::Error) -> ConversationError {
    ConversationError::Db(e.to_string())
}
