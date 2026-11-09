//! 交互写回（整合计划 Batch 3C）：响应卡交互的权威落库与生命周期。
//!
//! ## 职责
//!
//! - 接收卡片交互（accept / reject / submit / defer / decision），落
//!   `ai.conversation_interaction`（0005）——**权威终态**；
//! - 同事务把消息信封 `metadata.envelope.status` 终态化并写回记录号
//!   （回读投影：刷新/重进/他设备看到同一终态，不回退 presented——A-21）；
//! - 生命周期规则：一线程卡一生命周期（UNIQUE message_id）；终态幂等
//!   （同 action 重提交返回既有记录）；换 action 409（A-20 单向）。
//!
//! ## 不做
//!
//! - 不做业务生效（`recorded ≠ 生效`：接受一张推荐卡不改任何业务对象——
//!   正式业务变更由对应业务流负责，卡面文案已锁「已记录」语义）；
//! - 不收草稿/半答（Question 草稿存前端本地，A-22）；
//! - 不做 AI 冒充（decided_by 恒 = 会话属主，属主校验在端点层）。
//!
//! ## 生命周期状态
//!
//! 服务端只落**终态**（accepted/rejected/deferred/submitted/resolved）；
//! presented/in-progress 等中间态是前端表现态（Figma 委托书 §6 自注：
//! presentation states, not a business entity state machine）。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

/// 请求侧动作词表（`0005` CHECK 同源）。
pub const INTERACTION_ACTIONS: [&str; 5] = ["accept", "reject", "submit", "defer", "decision"];

/// 单条 reason / option_id 长度上限。
pub const MAX_INTERACTION_TEXT_CHARS: usize = 2_000;

/// 交互记录视图（REST 直接序列化）。
#[derive(Debug, Clone, Serialize)]
pub struct InteractionView {
    pub id: Uuid,
    pub message_id: Uuid,
    pub conversation_id: Uuid,
    pub action: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<serde_json::Value>,
    pub decided_by: Uuid,
    pub reference: String,
    pub created_at: DateTime<Utc>,
    /// 幂等重放标记：true = 本次为既有记录的幂等返回（非新建）。
    pub replayed: bool,
}

/// 写回失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InteractionError {
    /// 会话 / 消息不存在或跨属主（上层 404，防探测同形）。
    #[error("interaction target not found")]
    NotFound,
    /// 消息没有交互信封（纯文本消息不可交互）。
    #[error("message has no interaction envelope")]
    NotInteractive,
    /// 校验失败（400）。
    #[error("{0}")]
    Validation(String),
    /// 生命周期冲突：终态换 action（409）。
    #[error("interaction already terminal: {0}")]
    Conflict(&'static str),
    #[error("database error: {0}")]
    Db(String),
}

/// 动作 → 终态映射（accept→accepted / reject→rejected / submit→submitted /
/// defer→deferred / decision→resolved）。
fn status_for(action: &str) -> &'static str {
    match action {
        "accept" => "accepted",
        "reject" => "rejected",
        "submit" => "submitted",
        "defer" => "deferred",
        "decision" => "resolved",
        _ => unreachable!("action validated by caller"),
    }
}

fn db(e: sqlx::Error) -> InteractionError {
    InteractionError::Db(e.to_string())
}

/// 交互写回请求。
#[derive(Debug, Clone)]
pub struct InteractionRequest {
    pub action: String,
    pub reason: Option<String>,
    pub option_id: Option<String>,
    pub answer: Option<serde_json::Value>,
}

/// 提交互（权威落库 + 信封终态化，同事务）。
///
/// 生命周期：
/// - 既有记录 + 同 action → 幂等返回（`replayed = true`）；
/// - 既有记录 + 异 action → [`InteractionError::Conflict`]（409）；
/// - 无记录 + 信封终态（异常态，如手工改库）→ [`InteractionError::Conflict`]。
///
/// 四反馈语义（A-16~A-19）：
/// - reject 必须带非空 reason（缺 → 400）；
/// - decision 必须带 option_id 且 answer.confirmed == true（本人确认勾选，
///   服务端复验——缺 → 400，`recorded ≠ 生效`）。
#[allow(clippy::too_many_arguments)]
pub async fn submit_interaction(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    conversation_id: Uuid,
    message_id: Uuid,
    principal_id: Uuid,
    request: &InteractionRequest,
) -> Result<InteractionView, InteractionError> {
    let action = request.action.as_str();
    if !INTERACTION_ACTIONS.contains(&action) {
        return Err(InteractionError::Validation(format!(
            "action must be one of {}",
            INTERACTION_ACTIONS.join("/")
        )));
    }
    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty());
    if action == "reject" && reason.is_none() {
        return Err(InteractionError::Validation(
            "reject requires a non-empty reason".to_string(),
        ));
    }
    if let Some(text) = reason {
        if text.chars().count() > MAX_INTERACTION_TEXT_CHARS {
            return Err(InteractionError::Validation(format!(
                "reason must be at most {MAX_INTERACTION_TEXT_CHARS} characters"
            )));
        }
    }
    let option_id = request
        .option_id
        .as_deref()
        .map(str::trim)
        .filter(|o| !o.is_empty());
    if action == "decision" {
        // 本人确认服务端复验（A-18）：前端勾选确认以 answer.confirmed 声明。
        let confirmed = request
            .answer
            .as_ref()
            .and_then(|a| a.get("confirmed"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !confirmed {
            return Err(InteractionError::Validation(
                "decision requires explicit personal confirmation".to_string(),
            ));
        }
        if option_id.is_none() {
            return Err(InteractionError::Validation(
                "decision requires an option".to_string(),
            ));
        }
    }

    // 幂等 / 冲突预判（终态即封盘；同 action 幂等重放）。
    let existing = sqlx::query(
        "SELECT id, message_id, conversation_id, action, status, reason, option_id, \
                answer, decided_by, reference, created_at \
         FROM ai.conversation_interaction \
         WHERE tenant_id = $1 AND project_id = $2 AND conversation_id = $3 AND message_id = $4",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(conversation_id)
    .bind(message_id)
    .fetch_optional(pool)
    .await
    .map_err(db)?;
    if let Some(row) = existing {
        let existing_action: String = row.get("action");
        if existing_action == action {
            return Ok(row_to_view(&row, true));
        }
        return Err(InteractionError::Conflict("interaction_already_terminal"));
    }

    // 目标校验：消息必须属于本会话、属主匹配、且带交互信封（纯文本不可交互）。
    let target = sqlx::query(
        "SELECT c.principal_id, m.metadata \
         FROM ai.conversation_message m \
         JOIN ai.conversation c ON c.id = m.conversation_id \
         WHERE m.id = $1 AND m.conversation_id = $2 \
           AND c.tenant_id = $3 AND c.project_id = $4 AND c.principal_id = $5",
    )
    .bind(message_id)
    .bind(conversation_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await
    .map_err(db)?;
    let (owner, metadata): (Uuid, serde_json::Value) = match target {
        Some(row) => (row.get("principal_id"), row.get("metadata")),
        None => return Err(InteractionError::NotFound),
    };
    debug_assert_eq!(owner, principal_id, "owner filter binds principal_id");
    let envelope_status = metadata
        .get("envelope")
        .and_then(|e| e.get("status"))
        .and_then(serde_json::Value::as_str);
    let Some(envelope_status) = envelope_status else {
        return Err(InteractionError::NotInteractive);
    };
    if crate::ai_interaction_envelope::TERMINAL_STATUSES.contains(&envelope_status) {
        return Err(InteractionError::Conflict("interaction_already_terminal"));
    }

    let status = status_for(action);
    let mut tx = pool.begin().await.map_err(db)?;

    // 权威行。reference 冲突概率可忽略；唯一键兜底重试一次即可（不引入循环）。
    let interaction_id = Uuid::new_v4();
    let reference = format!("INT-{}", interaction_id.simple().to_string()[..8].to_uppercase());
    let created_at: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO ai.conversation_interaction \
             (tenant_id, project_id, conversation_id, message_id, action, status, \
              reason, option_id, answer, decided_by, reference) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
         RETURNING created_at",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(conversation_id)
    .bind(message_id)
    .bind(action)
    .bind(status)
    .bind(reason)
    .bind(option_id)
    .bind(request.answer.clone())
    .bind(principal_id)
    .bind(&reference)
    .fetch_one(&mut *tx)
    .await
    .map_err(db)?;

    // 回读投影（同事务）：信封 status 终态化 + 记录号/结果写回。
    sqlx::query(
        "UPDATE ai.conversation_message \
         SET metadata = jsonb_set( \
                 jsonb_set(metadata, '{envelope,status}', to_jsonb($3::text)), \
                 '{envelope,result}', \
                 jsonb_build_object('recorded', true, 'reference', $4::text, \
                                    'action', $5::text, 'at', $6::timestamptz) \
         ) \
         WHERE id = $1 AND conversation_id = $2",
    )
    .bind(message_id)
    .bind(conversation_id)
    .bind(status)
    .bind(&reference)
    .bind(action)
    .bind(created_at)
    .execute(&mut *tx)
    .await
    .map_err(db)?;

    tx.commit().await.map_err(db)?;

    Ok(InteractionView {
        id: interaction_id,
        message_id,
        conversation_id,
        action: action.to_string(),
        status: status.to_string(),
        reason: reason.map(str::to_string),
        option_id: option_id.map(str::to_string),
        answer: request.answer.clone(),
        decided_by: principal_id,
        reference,
        created_at,
        replayed: false,
    })
}

fn row_to_view(row: &sqlx::postgres::PgRow, replayed: bool) -> InteractionView {
    InteractionView {
        id: row.get("id"),
        message_id: row.get("message_id"),
        conversation_id: row.get("conversation_id"),
        action: row.get("action"),
        status: row.get("status"),
        reason: row.get("reason"),
        option_id: row.get("option_id"),
        answer: row.get("answer"),
        decided_by: row.get("decided_by"),
        reference: row.get("reference"),
        created_at: row.get("created_at"),
        replayed,
    }
}
