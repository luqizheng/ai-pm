//! 图投影层：权限裁剪 + 计数重算 + 一致性标记。
//!
//! 依据 docs/visualization/ai-pm-viz-14-v0-3-projection-consistency-design-v1.md：
//! 权限过滤必须在**业务查询层**与**图形投影层**双层执行；前端隐藏不能替代服务端过滤。
//!
//! 本模块是**纯函数**（无 DB、无 IO）：调用方先把"调用者可见的对象集合"解析出来
//! （由 [`ObjectResolver`] 实现负责），投影层据此裁剪 Spec。
//!
//! ## 泄露面（14 号 §4 的 11 项在本模块的落点）
//!
//! - 节点不可见 → 整节点移除（连带其全部入边/出边）；
//! - 边任一端不可见 → **整边丢弃，不输出半边**；
//! - 计数在裁剪**之后**重算，且**只暴露可见范围内的规模**——不提供「全量规模」
//!   字段，否则两个权限不同的人对比计数即可探测无权对象的存在；
//! - `meta.provenance.source_object_ids` 同样裁剪——它列着来源对象 id，是泄露面；
//! - 悬空边（端点因**数据缺失**而非权限裁剪而不在图中）保留并告警，因为那是
//!   数据完整性问题，用户应知道图不完整。

use std::collections::{HashMap, HashSet};

use pm_domain::visualization::{ObjectResolver, VizEdge, VizNode, VizSpec};

/// 一致性状态（14 号 §5.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Consistency {
    /// 数据版本一致（来自单条 SQL / CTE 的一致快照）。
    Current,
    /// 混合时点数据（跨多个权威来源分别读取）。
    MixedRevision,
}

impl Consistency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::MixedRevision => "mixed-revision",
        }
    }
}

/// 读取策略——调用方声明本次取数是否来自**同一条语句 / 同一一致性快照**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadStrategy {
    /// 单条 SQL 或单条带 CTE 的语句 → 一致性由单语句快照天然保证。
    SingleStatement,
    /// 多条独立语句 → 无法证明跨源统一时点。
    MultiStatement,
}

/// 可见范围内的规模（**只有可见部分**，不提供全量规模）。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VisibleCounts {
    /// 可见节点数。
    pub nodes: usize,
    /// 可见边数。
    pub edges: usize,
    /// 每个可见节点的可见度数（入 + 出）。
    pub degrees: HashMap<String, usize>,
}

/// 投影报告。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectionReport {
    /// 裁剪后的图（可直接交给前端渲染）。
    pub spec: VizSpec,
    /// 可见范围内的规模。
    pub counts: VisibleCounts,
    /// 一致性状态。
    pub consistency: Consistency,
    /// 因权限裁剪而移除的节点数（**仅内部审计用，不得回传前端**——它是全量规模的一
    /// 部分，会成为探测通道）。
    #[serde(skip)]
    pub dropped_nodes: usize,
    /// 因权限裁剪而丢弃的边数（同上，不回传前端）。
    #[serde(skip)]
    pub dropped_edges: usize,
    /// 悬空边 id（端点因数据缺失而不在图中——**不是**权限裁剪）。
    pub dangling_edges: Vec<String>,
}

impl ProjectionReport {
    /// 投影是否发生了裁剪（审计用）。
    pub fn was_trimmed(&self) -> bool {
        self.dropped_nodes > 0 || self.dropped_edges > 0
    }
}

/// 投影上下文。
///
/// **刻意不作为投影参数**：调用者（租户 / 主体 / 项目）已由依赖注入绑定在
/// [`ObjectResolver`] 实例上——投影层再传一遍只会制造"两边可能不是同一个调用者"
/// 的隐患。实现方在构造 resolver 时绑定调用者，投影层即可信任其判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionContext {
    /// 调用者租户。
    pub tenant_id: uuid::Uuid,
    /// 调用者主体。
    pub principal_id: uuid::Uuid,
    /// 目标项目。
    pub project_id: uuid::Uuid,
}

/// 投影一个 Spec：裁剪不可见对象 + 重算计数 + 标记一致性。
///
/// `resolver` 用于判定「对象是否存在**且**调用者可见」——无权一律按不存在处理
/// （与仓库 404 同形惯例一致，不泄露存在性）。
pub fn project_spec(
    spec: &VizSpec,
    resolver: &dyn ObjectResolver,
    strategy: ReadStrategy,
) -> ProjectionReport {
    // ── L1/L2-a：逐节点判定可见性
    let mut visible_nodes: Vec<&VizNode> = Vec::new();
    let mut visible_node_ids: HashSet<&str> = HashSet::new();
    let mut invisible_ids: HashSet<&str> = HashSet::new();

    for node in &spec.nodes {
        let visible = match &node.source_ref {
            Some(src) => resolver.exists_and_visible(&src.domain, &src.r#type, &src.id),
            // 无溯源节点：无从判定可见性，**保留**（它承载的是"未核实"而非"无权"）
            None => true,
        };
        if visible {
            visible_nodes.push(node);
            visible_node_ids.insert(node.id.as_str());
        } else {
            invisible_ids.insert(node.id.as_str());
        }
    }

    // ── L2-c：边任一端不可见 → 整边丢弃（不留半边）
    let mut visible_edges: Vec<&VizEdge> = Vec::new();
    let mut dangling_edges: Vec<String> = Vec::new();
    let mut dropped_edges = 0usize;

    for edge in &spec.edges {
        let from_visible = visible_node_ids.contains(edge.from.as_str());
        let to_visible = visible_node_ids.contains(edge.to.as_str());
        if from_visible && to_visible {
            visible_edges.push(edge);
            continue;
        }
        // 端点是否因权限被裁剪？
        let from_cut = invisible_ids.contains(edge.from.as_str());
        let to_cut = invisible_ids.contains(edge.to.as_str());
        if from_cut || to_cut {
            // 权限裁剪 → 整边丢弃，**不记入任何对外可见的计数**
            dropped_edges += 1;
        } else {
            // 端点压根不在图里 → 数据缺失（悬空边），保留并告警
            dangling_edges.push(edge.id.clone());
        }
    }

    // ── 组装裁剪后的 Spec
    let mut projected = spec.clone();
    projected.nodes = visible_nodes.iter().map(|n| (*n).clone()).collect();
    projected.edges = visible_edges.iter().map(|e| (*e).clone()).collect();
    trim_provenance(&mut projected);

    // ── L2-d：计数在裁剪后重算（只反映可见范围）
    let mut degrees: HashMap<String, usize> = HashMap::new();
    for node in &visible_nodes {
        degrees.insert(node.id.clone(), 0);
    }
    for edge in &visible_edges {
        *degrees.entry(edge.from.clone()).or_insert(0) += 1;
        *degrees.entry(edge.to.clone()).or_insert(0) += 1;
    }
    let counts = VisibleCounts {
        nodes: projected.nodes.len(),
        edges: projected.edges.len(),
        degrees,
    };

    // ── L2-f：一致性标记（无法证明时一律 mixed-revision，fail-closed）
    let consistency = match strategy {
        ReadStrategy::SingleStatement => Consistency::Current,
        ReadStrategy::MultiStatement => Consistency::MixedRevision,
    };

    // 裁剪计数在 move 之前算好（projected 之后被移入报告）
    let dropped_nodes = spec.nodes.len() - projected.nodes.len();

    ProjectionReport {
        spec: projected,
        counts,
        consistency,
        dropped_nodes,
        dropped_edges,
        dangling_edges,
    }
}

/// 裁剪 provenance 里的来源对象列表——**它是泄露面**（列着来源对象 id）。
fn trim_provenance(spec: &mut VizSpec) {
    let Some(provenance) = spec.meta.provenance.as_mut() else {
        return;
    };
    let visible: HashSet<&str> = spec.nodes.iter().map(|n| n.id.as_str()).collect();
    // 来源对象 id 与节点 id 不是同一命名空间，无法逐一对应 → 只保留**在图内出现过**
    // 的条目（节点 source_ref.id 命中即保留）。
    let source_ids: HashSet<&str> = spec
        .nodes
        .iter()
        .filter_map(|n| n.source_ref.as_ref().map(|s| s.id.as_str()))
        .collect();
    provenance
        .source_object_ids
        .retain(|id| source_ids.contains(id.as_str()) || visible.contains(id.as_str()));
    provenance
        .source_revision_refs
        .retain(|reference| source_ids.iter().any(|id| reference.contains(id)));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 全部可见的判定器。
    struct AllVisible;
    impl ObjectResolver for AllVisible {
        fn exists_and_visible(&self, _d: &str, _t: &str, _i: &str) -> bool {
            true
        }
        fn relation_exists(&self, _f: &str, _t: &str, _r: &str) -> bool {
            true
        }
    }

    /// 指定 id 不可见的判定器。
    struct HiddenOne<'a>(&'a str);
    impl ObjectResolver for HiddenOne<'_> {
        fn exists_and_visible(&self, _d: &str, _t: &str, id: &str) -> bool {
            id != self.0
        }
        fn relation_exists(&self, _f: &str, _t: &str, _r: &str) -> bool {
            true
        }
    }

    fn node(id: &str, source_id: &str) -> VizNode {
        VizNode {
            id: id.to_string(),
            label: format!("节点 {id}"),
            kind: "requirement_revision".to_string(),
            status: None,
            nature: None,
            source_ref: Some(pm_domain::visualization::SourceRef::new(
                "delivery",
                "requirement",
                source_id,
            )),
            derived: None,
        }
    }

    fn edge(id: &str, from: &str, to: &str) -> VizEdge {
        VizEdge {
            id: id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            relation: "task.implements_requirement".to_string(),
            label: None,
            nature: None,
            derived: None,
        }
    }

    fn spec() -> VizSpec {
        VizSpec {
            spec_version: "1.0".to_string(),
            slot_key: "slot".to_string(),
            view: pm_domain::visualization::VizView {
                kind: "dag".to_string(),
                focus: vec![],
                lenses: vec![],
                layout_hint: None,
            },
            nodes: vec![node("n1", "R-1"), node("n2", "R-2"), node("n3", "R-3")],
            edges: vec![edge("e1", "n1", "n2"), edge("e2", "n2", "n3")],
            meta: pm_domain::visualization::VizMeta {
                title: None,
                lang: None,
                budget: pm_domain::visualization::Budget::Balanced,
                nature: None,
                registry_version: None,
                provenance: Some(pm_domain::visualization::Provenance {
                    source_domain: "delivery".to_string(),
                    source_object_ids: vec![
                        "R-1".to_string(),
                        "R-2".to_string(),
                        "R-9".to_string(),
                    ],
                    source_revision_refs: vec!["R-1@rev1".to_string(), "R-9@rev9".to_string()],
                    source_snapshot_hash: None,
                }),
            },
        }
    }

    #[test]
    fn all_visible_keeps_everything() {
        let report = project_spec(&spec(), &AllVisible, ReadStrategy::SingleStatement);
        assert_eq!(report.counts.nodes, 3);
        assert_eq!(report.counts.edges, 2);
        assert!(!report.was_trimmed());
        assert_eq!(report.consistency, Consistency::Current);
    }

    #[test]
    fn invisible_node_is_removed_with_its_edges() {
        let report = project_spec(&spec(), &HiddenOne("R-2"), ReadStrategy::SingleStatement);
        // n2 不可见 → n2 与其两条边（e1、e2）全部移除
        assert_eq!(report.counts.nodes, 2);
        assert_eq!(report.counts.edges, 0);
        assert_eq!(report.dropped_nodes, 1);
        assert_eq!(report.dropped_edges, 2);
    }

    #[test]
    fn no_half_edges_are_emitted() {
        let report = project_spec(&spec(), &HiddenOne("R-2"), ReadStrategy::SingleStatement);
        for e in &report.spec.edges {
            assert!(report.spec.nodes.iter().any(|n| n.id == e.from));
            assert!(report.spec.nodes.iter().any(|n| n.id == e.to));
        }
    }

    #[test]
    fn counts_reflect_only_visible_scope() {
        let report = project_spec(&spec(), &HiddenOne("R-3"), ReadStrategy::SingleStatement);
        assert_eq!(report.counts.nodes, 2);
        assert_eq!(report.counts.edges, 1);
        // 度数只算可见边
        assert_eq!(report.counts.degrees.get("n1"), Some(&1));
        assert_eq!(report.counts.degrees.get("n2"), Some(&1));
        assert!(!report.counts.degrees.contains_key("n3"));
    }

    #[test]
    fn provenance_is_trimmed_too() {
        let report = project_spec(&spec(), &HiddenOne("R-3"), ReadStrategy::SingleStatement);
        let provenance = report.spec.meta.provenance.expect("provenance");
        // R-9 从未出现在图内 → 不应出现在来源列表
        assert!(!provenance.source_object_ids.contains(&"R-9".to_string()));
        assert!(provenance.source_object_ids.contains(&"R-1".to_string()));
        assert!(!provenance
            .source_revision_refs
            .contains(&"R-9@rev9".to_string()));
    }

    #[test]
    fn dangling_edge_is_reported_when_endpoint_missing_from_data() {
        let mut s = spec();
        // e2 的端点 n3 根本不在图里（数据缺失，不是权限裁剪）
        s.nodes.retain(|n| n.id != "n3");
        let report = project_spec(&s, &AllVisible, ReadStrategy::SingleStatement);
        assert_eq!(report.dangling_edges, vec!["e2".to_string()]);
        assert_eq!(report.dropped_edges, 0, "数据缺失不算权限裁剪");
    }

    #[test]
    fn multi_statement_read_is_marked_mixed_revision() {
        let report = project_spec(&spec(), &AllVisible, ReadStrategy::MultiStatement);
        assert_eq!(report.consistency, Consistency::MixedRevision);
        assert_eq!(report.consistency.as_str(), "mixed-revision");
    }

    #[test]
    fn single_statement_read_is_marked_current() {
        let report = project_spec(&spec(), &AllVisible, ReadStrategy::SingleStatement);
        assert_eq!(report.consistency, Consistency::Current);
    }

    #[test]
    fn node_without_source_is_kept() {
        let mut s = spec();
        s.nodes[0].source_ref = None;
        let report = project_spec(&s, &HiddenOne("R-2"), ReadStrategy::SingleStatement);
        // 无溯源节点承载的是"未核实"而非"无权" → 保留
        assert!(report.spec.nodes.iter().any(|n| n.id == "n1"));
    }
}
