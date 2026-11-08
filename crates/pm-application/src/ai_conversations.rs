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
    /// DK-A（0065）：线程最近一次升级指向的访谈会话；NULL = 纯 chat 线程。
    pub discovery_session_id: Option<Uuid>,
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
                created_at, updated_at, last_message_at, discovery_session_id \
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
/// 留给内部 AI 链）。`metadata` 为观测面（assistant 回复写 model/latency），
/// REST 侧传 `{}`。会话不存在 / 跨属主 / 跨项目 → [`ConversationError::NotFound`]。
#[allow(clippy::too_many_arguments)]
pub async fn append_message(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    role: &str,
    content: &str,
    content_type: &str,
    metadata: serde_json::Value,
) -> Result<MessageView, ConversationError> {
    if !MESSAGE_ROLES.contains(&role) {
        return Err(ConversationError::Validation(format!(
            "role must be one of {}",
            MESSAGE_ROLES.join("/")
        )));
    }
    let content_type = content_type.trim();
    validate_content(content, content_type)?;

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

    let message = insert_message_in(
        &mut tx,
        conversation_id,
        role,
        content,
        content_type,
        metadata,
    )
    .await?;
    tx.commit().await.map_err(db)?;
    Ok(message)
}

/// 内容校验（[`append_message`] 与 DK-A 载体判定 `conversation_router` 同一口径）。
pub(crate) fn validate_content(content: &str, content_type: &str) -> Result<(), ConversationError> {
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
    Ok(())
}

/// 事务内落一条消息并刷新会话 `updated_at` / `last_message_at`（单点 INSERT）。
///
/// 前置：调用方已在同事务内对会话行 `FOR UPDATE` 并完成属主校验、内容校验
/// （[`append_message`] 与 DK-A `conversation_router` 共用）。
pub(crate) async fn insert_message_in(
    conn: &mut sqlx::PgConnection,
    conversation_id: Uuid,
    role: &str,
    content: &str,
    content_type: &str,
    metadata: serde_json::Value,
) -> Result<MessageView, ConversationError> {
    // now() 在 Postgres 里是事务时间戳：消息 created_at 与会话刷新值同源。
    let row = sqlx::query(
        "INSERT INTO ai.conversation_message \
             (conversation_id, role, content, content_type, metadata) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, conversation_id, role, content, content_type, metadata, created_at",
    )
    .bind(conversation_id)
    .bind(role)
    .bind(content)
    .bind(content_type)
    .bind(metadata)
    .fetch_one(&mut *conn)
    .await
    .map_err(db)?;
    sqlx::query(
        "UPDATE ai.conversation \
         SET updated_at = now(), last_message_at = now() \
         WHERE id = $1",
    )
    .bind(conversation_id)
    .execute(&mut *conn)
    .await
    .map_err(db)?;
    Ok(message_from_row(&row))
}

/// 列出会话内消息（`created_at, id` ASC；`limit` 缺省
/// [`DEFAULT_MESSAGE_LIMIT`]，截到 [`MAX_MESSAGE_LIMIT`]）。
///
/// 双向 keyset 游标：
/// - `before`：向后翻页，只取 `created_at < before`；
/// - `after`（+ 可选 `after_id`）：向前取增量，取
///   `created_at > after OR (created_at = after AND id > after_id)`；
///   不给 `after_id` 时退化为严格 `created_at > after`。
///   `after_id` 是同微秒消息的稳定决胜键（id 随机，但排序确定性由本函数
///   的 `created_at, id` 双键保证），只给 `after_id` 不给 `after` 是无效
///   组合；`before` 与 `after` 同时给出互斥 400（Validation）。
///
/// Batch G：`after` 组合是前端增量轮询的游标（清偿「>500 条新尾部不可达」）。
///
/// Batch H `latest`（首屏「最新窗口」）：true 时按 `(created_at, id)` DESC
/// 取 `limit` 行（即**最新**的 limit 条）再反转为 ASC 返回——前端首屏从
/// 最旧窗口起的历史问题就此清偿。**从简裁决**：`latest` 与 `before` /
/// `after` 任一同给一律 400（after 在 DESC 集内的语义复杂且无用例，不做
/// 组合语义）；`latest` 缺省 false（旧行为：ASC 取最旧 limit 条）。
#[allow(clippy::too_many_arguments)]
pub async fn list_messages(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    before: Option<DateTime<Utc>>,
    after: Option<DateTime<Utc>>,
    after_id: Option<Uuid>,
    limit: Option<i64>,
    latest: bool,
) -> Result<Vec<MessageView>, ConversationError> {
    if before.is_some() && after.is_some() {
        return Err(ConversationError::Validation(
            "before and after cursors are mutually exclusive".into(),
        ));
    }
    if after_id.is_some() && after.is_none() {
        return Err(ConversationError::Validation(
            "after_id requires the after cursor".into(),
        ));
    }
    // Batch H 从简：latest 与 keyset 游标互斥（组合语义无用例，不做）。
    if latest && (before.is_some() || after.is_some()) {
        return Err(ConversationError::Validation(
            "latest cannot be combined with before/after cursors".into(),
        ));
    }
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

    let rows = if latest {
        // 最新窗口：DESC 取 limit 行再按双键 ASC 反转返回（SQL 内完成，
        // 返回顺序与默认分支一致）。
        sqlx::query(
            "SELECT id, conversation_id, role, content, content_type, metadata, created_at \
             FROM ( \
                 SELECT id, conversation_id, role, content, content_type, metadata, created_at \
                 FROM ai.conversation_message \
                 WHERE conversation_id = $1 \
                 ORDER BY created_at DESC, id DESC \
                 LIMIT $2 \
             ) latest_window \
             ORDER BY created_at ASC, id ASC",
        )
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(db)?
    } else {
        sqlx::query(
            "SELECT id, conversation_id, role, content, content_type, metadata, created_at \
             FROM ai.conversation_message \
             WHERE conversation_id = $1 \
               AND ($2::timestamptz IS NULL OR created_at < $2) \
               AND ($3::timestamptz IS NULL OR created_at > $3 \
                    OR (created_at = $3 AND $4::uuid IS NOT NULL AND id > $4)) \
             ORDER BY created_at ASC, id ASC \
             LIMIT $5",
        )
        .bind(conversation_id)
        .bind(before)
        .bind(after)
        .bind(after_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(db)?
    };
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
                created_at, updated_at, last_message_at, discovery_session_id \
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
        discovery_session_id: row.get("discovery_session_id"),
    }
}

pub(crate) fn message_from_row(row: &sqlx::postgres::PgRow) -> MessageView {
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

pub(crate) fn db(e: sqlx::Error) -> ConversationError {
    ConversationError::Db(e.to_string())
}
