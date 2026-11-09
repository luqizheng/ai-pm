//! 交互信封协议（整合计划 Batch 3B）：AI 消息 → 内联响应卡的载体。
//!
//! ## 载体
//!
//! 信封落在 `ai.conversation_message.metadata` 的 `envelope` 键下（**零迁移**
//! —— 该 jsonb 列 0060 已在）。消息级（不是线程级）：一张卡 = 一条 AI 消息。
//!
//! ## 为什么需要它
//!
//! 交互卡组件库（`AiResponseCard` + 8 类型响应卡）已完工，但真栈零消费——
//! `toTimelineMessage` 从不组装 `interaction`，后端也从不在消息里放信封。
//! 本模块补上**协议两端**：生产（服务端落信封）+ 消费（前端解析信封）。
//!
//! ## 白名单与 fail-closed
//!
//! - `type` ∈ 8 类白名单（与前端 `AiResponseType` 同源），白名单外 → **拒绝**
//!   （`EnvelopeError::UnknownType`）；
//! - `payload` 按类型校验必填字段，缺字段 → 拒绝；
//! - 校验失败一律**不落信封**（调用方落纯文本），**不伪造卡片**。
//!
//! ## 信封 ≠ 交互提交
//!
//! 本模块只管**展示**（卡能出现）。交互写回（接受/拒绝/提交的落库与终态）
//! 属 Batch 3C，独立端点与独立生命周期表。
//!
//! ## 生命周期状态
//!
//! `status` 用前端 `InteractionStatus` 的同一词表（表现态，非业务实体状态机
//! ——Figma 委托书 §6 自注）。服务端只**写入**状态，不在信封层做状态机迁移
//! （迁移规则属 Batch 3C）。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 8 类响应类型白名单（与前端 `AiResponseType` 同源）。
pub const RESPONSE_TYPES: [&str; 8] = [
    "informational",
    "recommendation",
    "proposal",
    "question",
    "decision",
    "artifact",
    "action-request",
    "execution-result",
];

/// 交互生命周期状态词表（表现态；与前端 `InteractionStatus` 同源）。
///
/// 终态（accepted/rejected/deferred/resolved/failed）由 Batch 3C 的写回链
/// 写入；服务端产卡时只写初始态（presented）。
pub const INTERACTION_STATUSES: [&str; 11] = [
    "created",
    "presented",
    "in-progress",
    "submitted",
    "validating",
    "processing",
    "accepted",
    "rejected",
    "deferred",
    "resolved",
    "failed",
];

/// 终态集合（Batch 3C 写回链的目标态；产卡链不写）。
pub const TERMINAL_STATUSES: [&str; 5] = ["accepted", "rejected", "deferred", "resolved", "failed"];

/// 信封错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvelopeError {
    /// 类型不在白名单内。
    #[error("unknown response type: {0}")]
    UnknownType(String),
    /// 状态不在词表内。
    #[error("unknown interaction status: {0}")]
    UnknownStatus(String),
    /// payload 缺必填字段或形态非法。
    #[error("invalid payload for {kind}: {detail}")]
    InvalidPayload { kind: String, detail: String },
    /// 信封结构非法（缺 type / status / payload 等基本键）。
    #[error("malformed envelope: {0}")]
    Malformed(String),
}

/// 交互信封（服务端产出与校验的形态）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionEnvelope {
    /// 信封 id（消息 id 或独立 uuid；此处由调用方给）。
    pub id: String,
    /// 8 类响应类型之一。
    #[serde(rename = "type")]
    pub kind: String,
    /// 交互生命周期状态。
    pub status: String,
    /// 产生者身份（AI 名 / 角色）。
    pub identity: Value,
    /// ISO 时间戳。
    pub created_at: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Value>,
    /// 类型专属载荷（按 type 校验）。
    pub payload: Value,
}

impl InteractionEnvelope {
    /// 落库形态：`{envelope: {...}}`（直接进 `metadata`）。
    pub fn to_metadata(&self) -> Value {
        serde_json::json!({ "envelope": self })
    }

    /// 从消息 metadata 读出信封（无 envelope 键 → None）。
    pub fn from_metadata(metadata: &Value) -> Option<Self> {
        let raw = metadata.get("envelope")?;
        serde_json::from_value(raw.clone()).ok()
    }

    /// 校验信封（白名单 + payload 必填）。
    ///
    /// **fail-closed**：校验不过 → 返回 Err，调用方落纯文本，**不伪造卡片**。
    pub fn validate(&self) -> Result<(), EnvelopeError> {
        if !RESPONSE_TYPES.contains(&self.kind.as_str()) {
            return Err(EnvelopeError::UnknownType(self.kind.clone()));
        }
        if !INTERACTION_STATUSES.contains(&self.status.as_str()) {
            return Err(EnvelopeError::UnknownStatus(self.status.clone()));
        }
        if !self.payload.is_object() {
            return Err(EnvelopeError::InvalidPayload {
                kind: self.kind.clone(),
                detail: "payload must be an object".to_string(),
            });
        }
        validate_payload(&self.kind, &self.payload)
    }
}

/// 各类型 payload 的必填字段（缺一即非法）。
///
/// 字段名与前端 `types.ts` 的 8 类 payload 接口同构（camelCase），逐一核对：
/// - informational: `contentMarkdown`
/// - recommendation: `summary`
/// - proposal: `proposalType`
/// - question: `groupTitle` + `questions`
/// - decision: `background` + `options`
/// - artifact: `artifactType` + `contentMarkdown`
/// - action-request: `action`
/// - execution-result: `agentName` + `executionState`
fn required_fields(envelope_type: &str) -> &'static [&'static str] {
    match envelope_type {
        "informational" => &["contentMarkdown"],
        "recommendation" => &["summary", "explanationMarkdown", "benefits", "risks"],
        "proposal" => &["proposalType", "changes", "impact", "related", "version"],
        "question" => &["groupTitle", "questions"],
        "decision" => &["background", "impact", "options"],
        "artifact" => &["artifactType", "contentMarkdown", "version"],
        "action-request" => &[
            "action",
            "target",
            "impact",
            "permission",
            "riskLevel",
            "statement",
        ],
        "execution-result" => &[
            "agentName",
            "taskRef",
            "executionState",
            "summary",
            "changedFiles",
            "logs",
        ],
        _ => &[],
    }
}

/// payload 校验：必填字段齐备 + 关键类型约束。
fn validate_payload(envelope_type: &str, payload: &Value) -> Result<(), EnvelopeError> {
    let obj = payload
        .as_object()
        .ok_or_else(|| EnvelopeError::InvalidPayload {
            kind: envelope_type.to_string(),
            detail: "payload must be an object".to_string(),
        })?;
    for field in required_fields(envelope_type) {
        if !obj.contains_key(*field) {
            return Err(EnvelopeError::InvalidPayload {
                kind: envelope_type.to_string(),
                detail: format!("missing required field `{field}`"),
            });
        }
    }
    // 类型专属约束：question 的 questions 必须是非空数组；decision 的
    // options 同理——空数组的卡没有可答/可选内容，属非法载荷。
    if envelope_type == "question" {
        let questions = obj.get("questions").and_then(Value::as_array);
        if questions.is_none_or(|items| items.is_empty()) {
            return Err(EnvelopeError::InvalidPayload {
                kind: envelope_type.to_string(),
                detail: "questions must be a non-empty array".to_string(),
            });
        }
    }
    if envelope_type == "decision" {
        let options = obj.get("options").and_then(Value::as_array);
        if options.is_none_or(|items| items.is_empty()) {
            return Err(EnvelopeError::InvalidPayload {
                kind: envelope_type.to_string(),
                detail: "options must be a non-empty array".to_string(),
            });
        }
    }
    Ok(())
}

/// 从消息 metadata 校验并取出信封。
///
/// 返回 `Err` 时调用方应落纯文本（fail-closed，不伪造卡片）。
pub fn validate_metadata_envelope(metadata: &Value) -> Result<InteractionEnvelope, EnvelopeError> {
    let envelope = InteractionEnvelope::from_metadata(metadata)
        .ok_or_else(|| EnvelopeError::Malformed("metadata has no envelope".to_string()))?;
    envelope.validate()?;
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_envelope(kind: &str, payload: Value) -> InteractionEnvelope {
        InteractionEnvelope {
            id: "env-1".to_string(),
            kind: kind.to_string(),
            status: "presented".to_string(),
            identity: json!({"name": "AI-PM"}),
            created_at: "2026-11-09T00:00:00Z".to_string(),
            title: "标题".to_string(),
            description: None,
            conclusion: None,
            evidence: None,
            payload,
        }
    }

    /// 白名单内 8 类 + 合法 payload → 校验通过。
    #[test]
    fn validates_all_eight_types_with_minimal_payloads() {
        // payload = 前端对应 payload 接口的必填字段全集（一一对应）。
        let cases = [
            ("informational", json!({"contentMarkdown": "正文"})),
            (
                "recommendation",
                json!({
                    "summary": "摘要",
                    "explanationMarkdown": "解释",
                    "benefits": [],
                    "risks": []
                }),
            ),
            (
                "proposal",
                json!({
                    "proposalType": "需求变更",
                    "changes": [],
                    "impact": [],
                    "related": [],
                    "version": {"revision": 1}
                }),
            ),
            (
                "question",
                json!({"groupTitle": "澄清", "questions": [{"id": "q1"}]}),
            ),
            (
                "decision",
                json!({"background": "背景", "impact": [], "options": [{"id": "o1"}]}),
            ),
            (
                "artifact",
                json!({
                    "artifactType": "requirement",
                    "contentMarkdown": "# 文档",
                    "version": {"revision": 1}
                }),
            ),
            (
                "action-request",
                json!({
                    "action": "部署",
                    "target": "prod",
                    "impact": [],
                    "permission": "deploy",
                    "riskLevel": "high",
                    "statement": "确认？"
                }),
            ),
            (
                "execution-result",
                json!({
                    "agentName": "runner-1",
                    "taskRef": "T-1",
                    "executionState": "succeeded",
                    "summary": "完成",
                    "changedFiles": [],
                    "logs": []
                }),
            ),
        ];
        for (kind, payload) in cases {
            let envelope = valid_envelope(kind, payload);
            assert!(envelope.validate().is_ok(), "{kind} must validate");
        }
    }

    /// 白名单外类型 → UnknownType（fail-closed：调用方落纯文本）。
    #[test]
    fn rejects_unknown_type() {
        let envelope = valid_envelope("chat-message", json!({}));
        assert_eq!(
            envelope.validate(),
            Err(EnvelopeError::UnknownType("chat-message".to_string()))
        );
    }

    /// 词表外状态 → UnknownStatus。
    #[test]
    fn rejects_unknown_status() {
        let mut envelope = valid_envelope("informational", json!({"contentMarkdown": "x"}));
        envelope.status = "expired".to_string();
        assert!(matches!(
            envelope.validate(),
            Err(EnvelopeError::UnknownStatus(_))
        ));
    }

    /// payload 缺必填字段 → InvalidPayload。
    #[test]
    fn rejects_missing_required_payload_fields() {
        let envelope = valid_envelope("question", json!({"groupTitle": "澄清"}));
        assert!(matches!(
            envelope.validate(),
            Err(EnvelopeError::InvalidPayload { .. })
        ));

        let envelope = valid_envelope("decision", json!({"background": "背景", "options": []}));
        assert!(matches!(
            envelope.validate(),
            Err(EnvelopeError::InvalidPayload { .. })
        ));

        let envelope = valid_envelope("informational", json!({}));
        assert!(matches!(
            envelope.validate(),
            Err(EnvelopeError::InvalidPayload { .. })
        ));
    }

    /// payload 非对象 → InvalidPayload。
    #[test]
    fn rejects_non_object_payload() {
        let envelope = valid_envelope("informational", json!("plain text"));
        assert!(matches!(
            envelope.validate(),
            Err(EnvelopeError::InvalidPayload { .. })
        ));
    }

    /// 落库形态 roundtrip：to_metadata → from_metadata 保真；wire 键名 = `type`。
    #[test]
    fn metadata_roundtrip_preserves_wire_shape() {
        let envelope = valid_envelope("question", json!({"groupTitle": "g", "questions": [1]}));
        let metadata = envelope.to_metadata();
        assert!(metadata.get("envelope").is_some());
        assert_eq!(metadata["envelope"]["type"], json!("question"));

        let parsed = InteractionEnvelope::from_metadata(&metadata).expect("parses");
        assert_eq!(parsed, envelope);
    }

    /// 无 envelope 键 → None；坏信封 → from_metadata None / validate Err。
    #[test]
    fn metadata_without_envelope_is_none() {
        assert!(InteractionEnvelope::from_metadata(&json!({})).is_none());
        assert!(InteractionEnvelope::from_metadata(&json!({"envelope": "broken"})).is_none());
    }

    /// validate_metadata_envelope：合法通过、缺键 Malformed、非法类型 Err。
    #[test]
    fn validate_metadata_envelope_fail_closed() {
        // 精简 fixture 本身不再合法（recommendation 必填 4 字段）——用 informational。
        let good = valid_envelope("informational", json!({"contentMarkdown": "正文"}));
        assert!(validate_metadata_envelope(&good.to_metadata()).is_ok());

        assert!(matches!(
            validate_metadata_envelope(&json!({})),
            Err(EnvelopeError::Malformed(_))
        ));

        let mut bad = valid_envelope("informational", json!({"contentMarkdown": "正文"}));
        bad.kind = "unknown-kind".to_string();
        assert!(validate_metadata_envelope(&bad.to_metadata()).is_err());
    }

    /// 终态集合与词表自洽（终态 ⊆ 词表；产卡链只写 presented）。
    #[test]
    fn terminal_statuses_are_subset_of_vocabulary() {
        for status in TERMINAL_STATUSES {
            assert!(INTERACTION_STATUSES.contains(&status));
        }
    }
}
