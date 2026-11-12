//! VizSpec：AI-PM 自有的统一图形数据模型（四段式：view / nodes / edges / meta）。
//!
//! 设计依据：docs/visualization/ai-pm-viz-03-visualization-spec-v1.md、
//! 12 号 V0-1 技术设计 §3。
//!
//! 硬约束：**零渲染器专属字段**。Spec 只表达语义与意图，几何、排版、动效由
//! Adapter 决定——这是防第三方技术绑定的前提（ADR-VIZ-003）。字段纯净性由
//! `scripts/viz-spec-lint.js` 在构建期守卫。

use crate::visualization::nature::Nature;
use crate::visualization::source_ref::{DerivedTrace, SourceRef};

/// Spec 版本（major.minor）。
///
/// 兼容规则（12 号 §3.3）：
/// - major 不同 → 拒绝（旧代码不得误读新字段，fail-closed）；
/// - major 相同 minor 更高 → 拒绝；
/// - minor 相同 → 直接读取。
pub const SPEC_VERSION: &str = "1.0";

/// 复杂度预算档位（节点/边上限见 [`SpecLimits`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Budget {
    /// ≤30 节点 / ≤45 边。
    Strict,
    /// ≤150 节点 / ≤300 边（默认）。
    Balanced,
    /// ≤500 节点 / ≤1000 边（导入类内容）。
    Faithful,
}

impl Budget {
    pub const ALL: [Self; 3] = [Self::Strict, Self::Balanced, Self::Faithful];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Balanced => "balanced",
            Self::Faithful => "faithful",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|b| b.as_str() == value)
    }

    /// 该档位允许的最大节点数。
    pub fn max_nodes(self) -> usize {
        match self {
            Self::Strict => 30,
            Self::Balanced => 150,
            Self::Faithful => 500,
        }
    }

    /// 该档位允许的最大边数。
    pub fn max_edges(self) -> usize {
        match self {
            Self::Strict => 45,
            Self::Balanced => 300,
            Self::Faithful => 1000,
        }
    }
}

/// 视图意图：渲染器据此选择模板。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VizView {
    /// 图形种类（受控词表，如 `trace-graph` / `dag` / `capability-map`）。
    pub kind: String,
    /// 焦点节点 id（accent 对象）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus: Vec<String>,
    /// 透镜（按关系类型/节点类型过滤高亮）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lenses: Vec<VizLens>,
    /// 布局意图（枚举字符串，非坐标）。渲染器可忽略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_hint: Option<String>,
}

/// 透镜：按条件过滤高亮。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VizLens {
    /// 透镜名称（图例与 Lens 菜单共用）。
    pub name: String,
    /// 匹配条件（如 `{"edge_type":"covers"}`）。
    #[serde(default)]
    pub r#match: serde_json::Value,
}

/// 图中节点：业务对象的呈现投影。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VizNode {
    /// 图内唯一 id。
    pub id: String,
    /// 显示标签（**纯文本**，渲染器负责转义；长度受 [`SpecLimits`] 约束）。
    pub label: String,
    /// 节点类型（受控词表，如 `requirement` / `design_revision` / `task`）。
    pub kind: String,
    /// 透传原领域状态（展示层语义）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// 节点性质；缺省继承 Artifact 级别。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nature: Option<Nature>,
    /// 来源引用（**业务上必填**）。
    ///
    /// 序列化为 `Option` 是刻意的：字段缺省时反序列化不失败，而是由校验器产出
    /// `E_SPEC_MISSING_SOURCE_REF`——问题码必须可达、可测、可 Golden 覆盖
    /// （若 serde 层直接拒绝，问题码将成为不可达分支）。
    #[serde(default)]
    pub source_ref: Option<SourceRef>,
    /// 间接推导标记（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<DerivedTrace>,
}

/// 图中边：业务关系的呈现投影。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VizEdge {
    /// 图内唯一 id。
    pub id: String,
    /// 起点节点 id。
    pub from: String,
    /// 终点节点 id。
    pub to: String,
    /// 关系键（命名空间受控字符串，如 `design.covers_requirement`）。
    pub relation: String,
    /// 边标签（纯文本）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// 边性质；缺省继承 Artifact 级别。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nature: Option<Nature>,
    /// 间接推导标记（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<DerivedTrace>,
}

/// 图的元信息。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VizMeta {
    /// 图标题。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// 标签语言（如 `zh-CN`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// 复杂度预算档位。
    pub budget: Budget,
    /// 图性质（Artifact 级）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nature: Option<Nature>,
    /// 生成该图时所用的 Relation Registry 版本。
    ///
    /// 参与 source_hash 计算（"影响图形解释的配置版本"），也用于版本落后提示。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_version: Option<String>,
    /// 来源快照信息（溯源与 stale 判定依据）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

/// 来源快照（与 `viz.artifact.provenance` 同构）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    /// 来源领域。
    pub source_domain: String,
    /// 来源对象 id 列表。
    pub source_object_ids: Vec<String>,
    /// 来源版本引用列表。
    #[serde(default)]
    pub source_revision_refs: Vec<String>,
    /// 来源快照哈希。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_snapshot_hash: Option<String>,
}

/// 一份完整的图形描述。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VizSpec {
    /// Spec 版本（见 [`SPEC_VERSION`] 与兼容规则）。
    pub spec_version: String,
    /// 图形挂点（`project:{id}/architecture-map` 形态）。
    pub slot_key: String,
    /// 视图意图。
    pub view: VizView,
    /// 节点。
    pub nodes: Vec<VizNode>,
    /// 边。
    pub edges: Vec<VizEdge>,
    /// 元信息。
    pub meta: VizMeta,
}

impl VizSpec {
    /// 节点 id 是否唯一。
    pub fn has_unique_node_ids(&self) -> bool {
        let mut ids: Vec<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        ids.len() == before
    }

    /// 边 id 是否唯一。
    pub fn has_unique_edge_ids(&self) -> bool {
        let mut ids: Vec<&str> = self.edges.iter().map(|e| e.id.as_str()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        ids.len() == before
    }

    /// 节点 id 集合（用于边的端点校验）。
    pub fn node_ids(&self) -> Vec<&str> {
        self.nodes.iter().map(|n| n.id.as_str()).collect()
    }
}

/// 尺寸上限（12 号 §7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecLimits {
    /// 单个 label 最大字符数。
    pub max_label_len: usize,
    /// 单个节点文本总长上限。
    pub max_node_text_len: usize,
    /// Spec 序列化后最大字节数。
    pub max_payload_bytes: usize,
    /// 边端点 id 最大长度。
    pub max_endpoint_len: usize,
}

impl Default for SpecLimits {
    fn default() -> Self {
        Self {
            max_label_len: 120,
            max_node_text_len: 200,
            max_payload_bytes: 256 * 1024,
            max_endpoint_len: 128,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> VizNode {
        VizNode {
            id: id.to_string(),
            label: "节点".to_string(),
            kind: "requirement".to_string(),
            status: None,
            nature: None,
            source_ref: Some(SourceRef::new("delivery", "requirement", id)),
            derived: None,
        }
    }

    fn spec() -> VizSpec {
        VizSpec {
            spec_version: SPEC_VERSION.to_string(),
            slot_key: "req-trace:R-042".to_string(),
            view: VizView {
                kind: "trace-graph".to_string(),
                focus: vec!["R-042".to_string()],
                lenses: vec![],
                layout_hint: Some("layered-lr".to_string()),
            },
            nodes: vec![node("R-042"), node("D-017")],
            edges: vec![VizEdge {
                id: "e1".to_string(),
                from: "R-042".to_string(),
                to: "D-017".to_string(),
                relation: "requirement.refines_design".to_string(),
                label: None,
                nature: None,
                derived: None,
            }],
            meta: VizMeta {
                title: Some("追踪图".to_string()),
                lang: Some("zh-CN".to_string()),
                budget: Budget::Balanced,
                nature: Some(Nature::Business),
                registry_version: Some("1".to_string()),
                provenance: None,
            },
        }
    }

    #[test]
    fn spec_round_trips() {
        let s = spec();
        let json = serde_json::to_string(&s).expect("serialize");
        let back: VizSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, s);
    }

    #[test]
    fn unique_id_detection() {
        let mut s = spec();
        assert!(s.has_unique_node_ids());
        assert!(s.has_unique_edge_ids());
        s.nodes.push(node("R-042"));
        assert!(!s.has_unique_node_ids());
    }

    #[test]
    fn budget_limits_match_spec() {
        assert_eq!(Budget::Strict.max_nodes(), 30);
        assert_eq!(Budget::Strict.max_edges(), 45);
        assert_eq!(Budget::Balanced.max_nodes(), 150);
        assert_eq!(Budget::Balanced.max_edges(), 300);
        assert_eq!(Budget::Faithful.max_nodes(), 500);
        assert_eq!(Budget::Faithful.max_edges(), 1000);
    }

    #[test]
    fn budget_round_trips() {
        for budget in Budget::ALL {
            assert_eq!(Budget::parse(budget.as_str()), Some(budget));
        }
    }

    #[test]
    fn optional_meta_fields_are_skipped() {
        let s = spec();
        let json = serde_json::to_string(&s.meta).expect("serialize");
        assert!(!json.contains("provenance"));
    }
}
