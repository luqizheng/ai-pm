//! 事件信封（V1.1 §13.1）。
//!
//! 全部跨服务事件以统一信封传输；`payload` 内引用对象一律 [`DomainRef`]
//! 四元组（§7.3），禁止内嵌 service 字段——信封顶层 `producer` 即服务来源
//! 元数据（ADR-07）。payload 纪律（§13.2）：不塞正文，只引用 + 摘要（≤256
//! 字符）+ hash。版本兼容（ADR-11）：`event_version` 整数递增 + additive-only，
//! 消费者容忍未知 version（跳过 + 告警）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 跨服务事件信封（§13.1 十一字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: Uuid,
    pub event_type: String,
    pub event_version: u32,
    pub occurred_at: DateTime<Utc>,
    /// 服务来源元数据（`kb-service` / `pc-service` / `platform`）——service
    /// 维度只在这一层承载，不下放进 payload 引用对象（ADR-07）。
    pub producer: String,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    /// 全链追踪：同一业务流程共享（如 intake_id）。
    pub correlation_id: String,
    /// 直接触发本事件的上下游事件 ID（如 kb.analysis.completed 的 event_id）。
    #[serde(default)]
    pub causation_id: Option<String>,
    /// 语义幂等键（§13.3 目录「幂等键业务部分」列）。
    pub idempotency_key: String,
    /// 事件正文（四元组引用 + 摘要 + hash；禁止内联正文）。
    pub payload: serde_json::Value,
}

impl EventEnvelope {
    /// 摘要长度上限（§13.2：不塞正文、只引用+摘要≤256+hash）。
    pub const SUMMARY_MAX_CHARS: usize = 256;

    /// 产线便捷构造：event_id/occurred_at 自动生成。
    pub fn new(
        event_type: &str,
        producer: &str,
        tenant_id: Uuid,
        project_id: Uuid,
        correlation_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            event_version: 1,
            occurred_at: Utc::now(),
            producer: producer.to_string(),
            tenant_id,
            project_id,
            correlation_id: correlation_id.into(),
            causation_id: None,
            idempotency_key: idempotency_key.into(),
            payload,
        }
    }

    /// 裁剪摘要到目录上限（§13.2）；供生产方在拼 payload 前归一化。
    pub fn clamp_summary(text: &str) -> &str {
        match text.char_indices().nth(Self::SUMMARY_MAX_CHARS) {
            Some((idx, _)) => &text[..idx],
            None => text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_clamped_to_256_chars() {
        let long = "汉".repeat(300);
        assert_eq!(EventEnvelope::clamp_summary(&long).chars().count(), 256);
        assert_eq!(EventEnvelope::clamp_summary("short"), "short");
    }

    #[test]
    fn envelope_roundtrips_with_null_causation() {
        let e = EventEnvelope::new(
            "kb.analysis.completed",
            "kb-service",
            Uuid::new_v4(),
            Uuid::new_v4(),
            "a1",
            "kb_analysis:1",
            serde_json::json!({}),
        );
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["event_version"], 1);
        assert_eq!(json["producer"], "kb-service");
        let back: EventEnvelope = serde_json::from_value(json).unwrap();
        assert_eq!(back.event_type, "kb.analysis.completed");
        assert_eq!(back.causation_id, None);
    }
}
