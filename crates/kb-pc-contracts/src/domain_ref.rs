//! 四元组跨服务引用本体（ADR-07，V1.1 §7.3；与 PC-01B §13.2 完全同构）。
//!
//! **唯一持久化引用结构**：`{target_domain, target_type, target_id,
//! target_revision_id}`。`target_id`/`target_revision_id` 为 TEXT（跨域身份
//! 形态不一；域内 UUID 亦以字符串承载）。有 Revision 机制的对象必填
//! revision 引用，空缺必须记录原因（原因记录归各领域承载表，如
//! `pc.source.revision_missing_reason`，本类型不做 reasons 表达）。
//!
//! ADR-07：service 仅作传输信封级元数据（事件顶层 `producer` /
//! `intake_task.target_service` 列），**禁止进入任何持久化引用结构**——
//! 本结构刻意不含 service 字段；序列化规范（serde）以 wire 形态冻结：
//! 字段名 snake_case、`target_revision_id` 缺失序列化为 `null`（结构稳定，
//! 便于对账与 hash）。

use crate::registry::RegistryError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 四元组跨服务引用（§7.3；禁止内嵌 service 字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainRef {
    pub target_domain: String,
    pub target_type: String,
    pub target_id: String,
    /// Revision 级引用；有 Revision 机制的对象必填，空缺必须记录原因。
    /// 序列化为显式 `null`（不省略键），保证 wire 形态稳定可对账。
    #[serde(default)]
    pub target_revision_id: Option<String>,
}

impl DomainRef {
    /// 构造并做最小校验（三元非空，`target_domain` 必须可解析——解析失败
    /// 直接拒绝构造，防未登记 domain 流入持久化）。
    ///
    /// 未开放通道的 domain（如 `design`/`product_plan`）允许构造：引用的
    /// **合法性**（通道状态）由消费方按 [`crate::registry`] 在业务校验时
    /// 裁决，构造层只拦「解析不了」的值。
    pub fn new(
        target_domain: impl Into<String>,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        target_revision_id: Option<String>,
    ) -> Result<Self, RegistryError> {
        let r = Self {
            target_domain: target_domain.into(),
            target_type: target_type.into(),
            target_id: target_id.into(),
            target_revision_id,
        };
        r.validate()?;
        Ok(r)
    }

    /// 已注册 domain 的便捷构造（`FromStr` 失败即 panic 的场景不要用本函数
    /// 之外的途径拼字符串）。
    pub fn for_domain(
        domain: crate::registry::TargetDomain,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        target_revision_id: Option<String>,
    ) -> Self {
        Self {
            target_domain: domain.as_str().to_string(),
            target_type: target_type.into(),
            target_id: target_id.into(),
            target_revision_id,
        }
    }

    fn validate(&self) -> Result<(), RegistryError> {
        if self.target_domain.trim().is_empty()
            || self.target_type.trim().is_empty()
            || self.target_id.trim().is_empty()
        {
            return Err(RegistryError::InvalidRef(
                "domain/type/id must be non-empty".to_string(),
            ));
        }
        // domain 必须可解析（注册表登记全集）；解析失败 → 告警错误类型。
        crate::registry::parse_domain(&self.target_domain)?;
        Ok(())
    }

    /// 事件 payload 摘要纪律（§13.2）：引用对象只进四元组 + hash，摘要
    /// ≤256 字符由**调用方**裁剪；本方法供日志/审计输出稳定形态。
    pub fn display_key(&self) -> String {
        format!(
            "{}:{}/{}@{}",
            self.target_domain,
            self.target_type,
            self.target_id,
            self.target_revision_id.as_deref().unwrap_or("-")
        )
    }
}

impl fmt::Display for DomainRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{RegistryError, TargetDomain};

    #[test]
    fn serde_wire_shape_is_frozen() {
        let r = DomainRef::for_domain(
            TargetDomain::KnowledgeBase,
            "memory_revision",
            "0193f4a0",
            Some("0193f4a1".to_string()),
        );
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "target_domain": "knowledge_base",
                "target_type": "memory_revision",
                "target_id": "0193f4a0",
                "target_revision_id": "0193f4a1"
            })
        );
        // 缺失 revision → 显式 null（不是省略键）。
        let r2 = DomainRef::for_domain(TargetDomain::Requirement, "requirement", "r-1", None);
        let json2 = serde_json::to_value(&r2).unwrap();
        assert_eq!(json2["target_revision_id"], serde_json::Value::Null);
        assert!(json2.get("target_revision_id").is_some());
        // 反序列化容忍缺失字段（additive-only 兼容面）。
        let back: DomainRef =
            serde_json::from_value(serde_json::json!({"target_domain": "pc", "target_type": "proposal", "target_id": "p1"}))
                .unwrap();
        assert_eq!(back.target_revision_id, None);
    }

    #[test]
    fn unknown_domain_is_rejected_at_construction() {
        let err = DomainRef::new("no_such_domain", "t", "id", None).unwrap_err();
        assert!(matches!(err, RegistryError::UnknownDomain(_)));
    }

    #[test]
    fn empty_triplet_is_rejected() {
        assert!(DomainRef::new("pc", "", "id", None).is_err());
        assert!(DomainRef::new("pc", "t", "  ", None).is_err());
    }

    #[test]
    fn display_key_is_stable() {
        let r = DomainRef::for_domain(TargetDomain::ProjectGoal, "project_goal", "g1", None);
        assert_eq!(r.display_key(), "project_goal:project_goal/g1@-");
    }
}
