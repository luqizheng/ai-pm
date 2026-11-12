//! 来源引用：图上每个节点/边指向的正式知识出处。
//!
//! 字段与 `kb.memory_ref` 同构（`migrations/0001_baseline.sql:2914`），
//! 去掉 `ref_role`——角色属业务语义，由边关系表达，而非引用本身。
//!
//! 溯源是**强制**的：节点的 `source_ref` 缺失即校验失败（`E_SPEC_MISSING_SOURCE_REF`）。
//! 无溯源的节点必须显式标注 `unlinked`，不得静默冒充已核实知识。

/// 单个来源引用（对象 + 版本 + 定位）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceRef {
    /// 来源领域：`delivery` / `design` / `pc` / `ai` / `kb` / `plan`。
    pub domain: String,
    /// 来源类型：`requirement` / `design_revision` / `memory_ref` …。
    #[serde(rename = "type")]
    pub r#type: String,
    /// 来源对象 id（业务键或 uuid 的字符串形态）。
    pub id: String,
    /// 来源版本 id；关系绑定到具体修订时必填。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<String>,
    /// 来源定位信息（文档路径、章节、字段 …）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<serde_json::Value>,
    /// 来源内容哈希（裸小写 hex sha256），用于 stale 判定。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

impl SourceRef {
    pub fn new(
        domain: impl Into<String>,
        r#type: impl Into<String>,
        id: impl Into<String>,
    ) -> Self {
        Self {
            domain: domain.into(),
            r#type: r#type.into(),
            id: id.into(),
            revision_id: None,
            locator: None,
            content_hash: None,
        }
    }

    pub fn with_revision(mut self, revision_id: impl Into<String>) -> Self {
        self.revision_id = Some(revision_id.into());
        self
    }

    pub fn with_content_hash(mut self, content_hash: impl Into<String>) -> Self {
        self.content_hash = Some(content_hash.into());
        self
    }

    /// 排序键：按 (domain, type, id, revision) 稳定排序，
    /// 保证 source_hash 与输入顺序无关（对齐 `review/anchor.rs` 的排序先例）。
    pub fn sort_key(&self) -> (&str, &str, &str, &str) {
        (
            self.domain.as_str(),
            self.r#type.as_str(),
            self.id.as_str(),
            self.revision_id.as_deref().unwrap_or(""),
        )
    }
}

/// 间接推导标记（推导方式维度，与 `Nature` 正交）。
///
/// 二跳拼接的边**必须**携带 `direct=false` + `via` + `semantic_confidence`，
/// 图上以虚线与「经 X 推导」呈现，影响分析归入 Indirect 分组
/// （14 号 §6、ADR-VIZ-005）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DerivedTrace {
    /// 是否为直接业务关系。二跳拼接恒为 `false`。
    pub direct: bool,
    /// 推导路径上的中间对象 id。
    pub via: Vec<String>,
    /// 语义可信度标记。`structurally_supported` 只声明**路径结构完整**，
    /// 不代表业务语义有 100% 确定性。
    pub semantic_confidence: String,
}

impl DerivedTrace {
    /// 结构性支撑（唯一的 V1 取值）。
    pub const STRUCTURALLY_SUPPORTED: &'static str = "structurally_supported";

    /// 构造一条间接推导标记。
    pub fn indirect(via: Vec<String>) -> Self {
        Self {
            direct: false,
            via,
            semantic_confidence: Self::STRUCTURALLY_SUPPORTED.to_string(),
        }
    }

    /// 推导标记是否结构合法：`direct=false` 且 `via` 非空。
    pub fn is_structurally_valid(&self) -> bool {
        !self.direct && !self.via.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_fields_are_skipped_when_absent() {
        let json = serde_json::to_string(&SourceRef::new("delivery", "requirement", "R-042"))
            .expect("serialize");
        assert_eq!(json, r#"{"domain":"delivery","type":"requirement","id":"R-042"}"#);
    }

    #[test]
    fn revision_and_hash_round_trip() {
        let src = SourceRef::new("delivery", "requirement", "R-042")
            .with_revision("rev-12")
            .with_content_hash("abc123");
        let back: SourceRef = serde_json::from_str(&serde_json::to_string(&src).unwrap()).unwrap();
        assert_eq!(back, src);
        assert_eq!(back.sort_key(), ("delivery", "requirement", "R-042", "rev-12"));
    }

    #[test]
    fn derived_trace_requires_non_empty_via() {
        assert!(DerivedTrace::indirect(vec!["R-042".into()]).is_structurally_valid());
        assert!(!DerivedTrace::indirect(vec![]).is_structurally_valid());
        let mut direct = DerivedTrace::indirect(vec!["R-042".into()]);
        direct.direct = true;
        assert!(!direct.is_structurally_valid());
    }

    #[test]
    fn sort_key_treats_missing_revision_as_empty() {
        let src = SourceRef::new("design", "design_revision", "D-1");
        assert_eq!(src.sort_key().3, "");
    }
}
