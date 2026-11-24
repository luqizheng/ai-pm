//! VizSpec 校验器：**纯函数 + 分级问题列表**。
//!
//! 设计依据：12 号 V0-1 技术设计 §4；问题码表见 [`codes`]。
//!
//! 形态约束：
//! - **纯函数**：无 IO、无 DB、无全局状态——判定所需的外部信息全部由
//!   [`ValidationContext`] 注入（存在性/权限由应用层实现 [`ObjectResolver`]）。
//! - **权限前置**：无权对象一律按「不存在」处理，错误信息不得出现「无权」字样
//!   （与仓库 404 同形惯例一致，避免探测差异）。
//! - **分级**：`E_` 阻断（不得入库/渲染）、`W_` 警示（可渲染但需标注）、`I_` 提示。

pub mod codes;

use std::collections::{HashMap, HashSet};

use crate::visualization::nature::Nature;
use crate::visualization::registry::RelationRegistry;
use crate::visualization::source_ref::DerivedTrace;
use crate::visualization::spec::{Budget, SpecLimits, VizSpec, SPEC_VERSION};

/// 校验严重级别。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// 提示。
    Info,
    /// 警示（可渲染，但需显式标注）。
    Warning,
    /// 阻断（不得入库 / 不得渲染）。
    Blocker,
}

/// 单条校验问题。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VizProblem {
    /// 问题码（见 [`codes`]）。
    pub code: String,
    /// 定位串，如 `nodes[3].label`。
    pub at: String,
    /// 人类可读说明。
    pub message: String,
    /// 可选修复建议（供 AI 修补循环消费）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// 校验报告。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidationReport {
    /// 问题列表（按校验顺序产生）。
    pub problems: Vec<VizProblem>,
}

impl ValidationReport {
    /// 是否存在阻断级问题。
    pub fn has_blockers(&self) -> bool {
        self.problems
            .iter()
            .any(|p| p.severity() == Severity::Blocker)
    }

    /// 阻断级问题。
    pub fn blocking(&self) -> Vec<&VizProblem> {
        self.problems
            .iter()
            .filter(|p| p.severity() == Severity::Blocker)
            .collect()
    }

    /// 是否全部通过（无任何问题）。
    pub fn is_clean(&self) -> bool {
        self.problems.is_empty()
    }
}

impl VizProblem {
    /// 级别：由问题码前缀决定。
    pub fn severity(&self) -> Severity {
        if self.code.starts_with('E') {
            Severity::Blocker
        } else if self.code.starts_with('W') {
            Severity::Warning
        } else {
            Severity::Info
        }
    }
}

/// 对象存在性与可见性判定（由应用层实现；pm-domain 不依赖 DB）。
///
/// **同步是刻意的**：可见性判定基于**应用层预取好的可见集合**做内存查表——
/// 校验器与投影层因此保持纯函数（无 IO、无 DB），且避免逐对象查库（N+1）。
/// 应用层负责「批量预取 → 构建实现」的异步环节（见 pm-application 的
/// `visibility_probe`）。
pub trait ObjectResolver {
    /// 对象是否存在**且调用者可见**。
    ///
    /// 无权对象必须返回 `false`（与不存在同形），不得泄露存在性。
    fn exists_and_visible(&self, domain: &str, r#type: &str, id: &str) -> bool;

    /// 关系是否存在（用于 `derived_trace` 的 `via` 校验）。
    fn relation_exists(&self, from: &str, to: &str, relation: &str) -> bool;
}

/// 校验上下文（全部外部依赖的注入口）。
pub struct ValidationContext<'a> {
    /// 关系词表。
    pub registry: &'a RelationRegistry,
    /// 对象解析器。
    pub resolver: &'a dyn ObjectResolver,
    /// 尺寸上限。
    pub limits: SpecLimits,
    /// 调用者可写的槽位白名单（空 = 不限制）。
    pub allowed_slots: &'a [String],
    /// 当前发布使用的 Registry 版本（用于版本落后提示）。
    pub current_registry_version: Option<&'a str>,
}

/// 收集 Spec 中需要做可见性判定的全部引用（`(domain, type, id)` 去重）。
///
/// 应用层用它**一次性批量预取**可见集合，再构建内存判定器——
/// 校验器与投影层因此不必逐对象查库。
///
/// 覆盖：节点 `source_ref`（含其 revision 指向的对象）与 `derived_trace.via`
/// 上的中间对象（间接推导路径上的对象同样可能无权）。
pub fn collect_visibility_refs(spec: &VizSpec) -> Vec<(String, String, String)> {
    let mut refs: Vec<(String, String, String)> = Vec::new();
    for node in &spec.nodes {
        if let Some(source) = &node.source_ref {
            refs.push((
                source.domain.clone(),
                source.r#type.clone(),
                source.id.clone(),
            ));
        }
    }
    for edge in &spec.edges {
        if let Some(derived) = &edge.derived {
            for via in &derived.via {
                refs.push((String::new(), String::new(), via.clone()));
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

/// 校验入口：**纯函数**。
pub fn validate(spec: &VizSpec, ctx: &ValidationContext<'_>) -> ValidationReport {
    let mut report = ValidationReport::default();

    validate_version(spec, ctx, &mut report);
    validate_slot(spec, ctx, &mut report);
    validate_budget(spec, ctx, &mut report);
    validate_payload_size(spec, ctx, &mut report);
    validate_nodes(spec, ctx, &mut report);
    validate_edges(spec, ctx, &mut report);
    validate_view(spec, &mut report);

    report
}

// ───────────────────────────────────────────── 分段校验

fn validate_version(spec: &VizSpec, ctx: &ValidationContext<'_>, report: &mut ValidationReport) {
    let (major, minor) = match parse_version(&spec.spec_version) {
        Some(v) => v,
        None => {
            push(
                report,
                codes::blocker::SPEC_VERSION_UNSUPPORTED,
                "spec_version",
                format!("无法解析的版本号：{}", spec.spec_version),
                Some(format!("使用版本号 {SPEC_VERSION}")),
            );
            return;
        }
    };
    let (cur_major, cur_minor) = parse_version(SPEC_VERSION).expect("SPEC_VERSION 合法");
    if major != cur_major {
        push(
            report,
            codes::blocker::SPEC_VERSION_UNSUPPORTED,
            "spec_version",
            format!(
                "major 版本不兼容：{}（当前支持 {SPEC_VERSION}）",
                spec.spec_version
            ),
            None,
        );
    } else if minor > cur_minor {
        push(
            report,
            codes::blocker::SPEC_VERSION_UNSUPPORTED,
            "spec_version",
            format!("minor 版本高于当前实现：{}", spec.spec_version),
            None,
        );
    }

    if let Some(current) = ctx.current_registry_version {
        if let Some(spec_version) = spec.meta.registry_version.as_deref() {
            if spec_version != current {
                push(
                    report,
                    codes::info::SPEC_REGISTRY_VERSION_STALE,
                    "meta.registry_version",
                    format!("词表版本 {spec_version} 落后于当前发布版本 {current}"),
                    None,
                );
            }
        }
    }
}

fn validate_slot(spec: &VizSpec, ctx: &ValidationContext<'_>, report: &mut ValidationReport) {
    if spec.slot_key.trim().is_empty() {
        push(
            report,
            codes::blocker::SPEC_SLOT_NOT_ALLOWED,
            "slot_key",
            "槽位键不得为空".to_string(),
            None,
        );
        return;
    }
    if !ctx.allowed_slots.is_empty() && !ctx.allowed_slots.contains(&spec.slot_key) {
        push(
            report,
            codes::blocker::SPEC_SLOT_NOT_ALLOWED,
            "slot_key",
            format!("槽位 {} 不在可写白名单内", spec.slot_key),
            None,
        );
    }
}

fn validate_budget(spec: &VizSpec, ctx: &ValidationContext<'_>, report: &mut ValidationReport) {
    let budget = spec.meta.budget;
    if spec.nodes.len() > budget.max_nodes() {
        push(
            report,
            codes::blocker::SPEC_BUDGET_EXCEEDED,
            "meta.budget",
            format!(
                "节点数 {} 超出 {} 档位上限 {}",
                spec.nodes.len(),
                budget.as_str(),
                budget.max_nodes()
            ),
            Some("拆分图形或提升 budget 档位".to_string()),
        );
    }
    if spec.edges.len() > budget.max_edges() {
        push(
            report,
            codes::blocker::SPEC_BUDGET_EXCEEDED,
            "meta.budget",
            format!(
                "边数 {} 超出 {} 档位上限 {}",
                spec.edges.len(),
                budget.as_str(),
                budget.max_edges()
            ),
            Some("拆分图形或提升 budget 档位".to_string()),
        );
    }
    let _ = ctx;
}

fn validate_payload_size(
    spec: &VizSpec,
    ctx: &ValidationContext<'_>,
    report: &mut ValidationReport,
) {
    let bytes = serde_json::to_vec(spec)
        .map(|b| b.len())
        .unwrap_or(usize::MAX);
    if bytes > ctx.limits.max_payload_bytes {
        push(
            report,
            codes::blocker::SPEC_PAYLOAD_TOO_LARGE,
            "meta",
            format!(
                "序列化后 {} 字节，超出上限 {}",
                bytes, ctx.limits.max_payload_bytes
            ),
            Some("减少节点/标签长度或拆分图形".to_string()),
        );
    }
}

fn validate_nodes(spec: &VizSpec, ctx: &ValidationContext<'_>, report: &mut ValidationReport) {
    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut seen_labels: HashMap<(&str, &str), usize> = HashMap::new();

    for (index, node) in spec.nodes.iter().enumerate() {
        let at = format!("nodes[{index}]");

        if !seen_ids.insert(node.id.as_str()) {
            push(
                report,
                codes::blocker::SPEC_DUPLICATE_NODE_ID,
                &format!("{at}.id"),
                format!("节点 id 重复：{}", node.id),
                Some("为每个节点分配唯一 id".to_string()),
            );
        }

        validate_label(&node.label, &format!("{at}.label"), ctx, report);

        let text_len =
            node.label.chars().count() + node.status.as_deref().map(str::len).unwrap_or(0);
        if text_len > ctx.limits.max_node_text_len {
            push(
                report,
                codes::blocker::SPEC_LABEL_TOO_LONG,
                &format!("{at}.label"),
                format!(
                    "节点文本总长 {text_len} 超出上限 {}",
                    ctx.limits.max_node_text_len
                ),
                Some("精简标签".to_string()),
            );
        }

        let source_ref = match &node.source_ref {
            None => {
                push(
                    report,
                    codes::blocker::SPEC_MISSING_SOURCE_REF,
                    &format!("{at}.source_ref"),
                    "节点缺少来源引用".to_string(),
                    Some("为节点补充 source_ref".to_string()),
                );
                None
            }
            Some(source_ref) if source_ref.id.trim().is_empty() => {
                push(
                    report,
                    codes::blocker::SPEC_MISSING_SOURCE_REF,
                    &format!("{at}.source_ref"),
                    "节点来源引用缺少对象 id".to_string(),
                    Some("补全 source_ref.id".to_string()),
                );
                None
            }
            Some(source_ref) => Some(source_ref),
        };
        if let Some(source_ref) = source_ref {
            if !ctx.resolver.exists_and_visible(
                &source_ref.domain,
                &source_ref.r#type,
                &source_ref.id,
            ) {
                push(
                    report,
                    codes::blocker::SPEC_UNKNOWN_OBJECT_REF,
                    &format!("{at}.source_ref"),
                    format!(
                        "来源对象不存在或不可见：{}/{}/{}",
                        source_ref.domain, source_ref.r#type, source_ref.id
                    ),
                    Some("移除该节点或改用可见对象".to_string()),
                );
            }
        }

        if !ctx.registry.has_relation_kind(&node.kind) {
            push(
                report,
                codes::blocker::SPEC_NODE_KIND_NOT_REGISTERED,
                &format!("{at}.kind"),
                format!("节点类型未登记：{}", node.kind),
                Some("使用已登记的节点类型".to_string()),
            );
        }

        if let Some(derived) = &node.derived {
            validate_derived(derived, &format!("{at}.derived"), report);
        }

        let label_key = (node.kind.as_str(), node.label.as_str());
        let counter = seen_labels.entry(label_key).or_insert(0);
        *counter += 1;
        if *counter == 2 {
            push(
                report,
                codes::warning::SPEC_DUPLICATE_LABEL,
                &format!("{at}.label"),
                format!("同类型重复标签：{} / {}", node.kind, node.label),
                None,
            );
        }
    }
}

fn validate_edges(spec: &VizSpec, ctx: &ValidationContext<'_>, report: &mut ValidationReport) {
    let node_ids: HashSet<&str> = spec.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut connected: HashSet<&str> = HashSet::new();

    for (index, edge) in spec.edges.iter().enumerate() {
        let at = format!("edges[{index}]");

        if !seen_ids.insert(edge.id.as_str()) {
            push(
                report,
                codes::blocker::SPEC_DUPLICATE_EDGE_ID,
                &format!("{at}.id"),
                format!("边 id 重复：{}", edge.id),
                Some("为每条边分配唯一 id".to_string()),
            );
        }

        if let Some(label) = &edge.label {
            validate_label(label, &format!("{at}.label"), ctx, report);
        }
        for (field, endpoint) in [("from", edge.from.as_str()), ("to", edge.to.as_str())] {
            if endpoint.len() > ctx.limits.max_endpoint_len {
                push(
                    report,
                    codes::blocker::SPEC_LABEL_TOO_LONG,
                    &format!("{at}.{field}"),
                    format!("端点 id 超长（{} 字符）", endpoint.len()),
                    Some("缩短端点 id".to_string()),
                );
            }
            if !node_ids.contains(endpoint) {
                let dangling = spec
                    .nodes
                    .iter()
                    .filter_map(|n| n.source_ref.as_ref())
                    .all(|src| src.id != endpoint);
                if dangling {
                    push(
                        report,
                        codes::blocker::SPEC_EDGE_ENDPOINT_UNKNOWN,
                        &format!("{at}.{field}"),
                        format!("边端点未命中任何节点：{endpoint}"),
                        Some("修正端点 id 或补充节点".to_string()),
                    );
                } else {
                    push(
                        report,
                        codes::warning::SPEC_DANGLING_EDGE,
                        &format!("{at}.{field}"),
                        format!("边端点因权限裁剪而缺失：{endpoint}"),
                        None,
                    );
                }
            } else {
                connected.insert(endpoint);
            }
        }

        match ctx.registry.relation(&edge.relation) {
            None => push(
                report,
                codes::blocker::SPEC_RELATION_NOT_REGISTERED,
                &format!("{at}.relation"),
                format!("关系未登记：{}", edge.relation),
                Some("使用词表中已登记的关系键".to_string()),
            ),
            Some(relation) => {
                if !relation.status.is_projectable() {
                    push(
                        report,
                        codes::blocker::SPEC_RELATION_NOT_APPROVED,
                        &format!("{at}.relation"),
                        format!(
                            "关系 {} 处于 {} 状态，不可投影",
                            edge.relation,
                            relation.status.as_str()
                        ),
                        Some("改用状态为 active 的关系".to_string()),
                    );
                }
                if relation.view_alias {
                    push(
                        report,
                        codes::blocker::SPEC_RELATION_NOT_REGISTERED,
                        &format!("{at}.relation"),
                        format!("关系 {} 是反向视图别名，不得作为独立边投影", edge.relation),
                        Some("改用其 reverse_view_of 指向的关系键".to_string()),
                    );
                }
                validate_endpoint_types(spec, edge, relation, &at, report);
            }
        }

        if let Some(derived) = &edge.derived {
            validate_derived(derived, &format!("{at}.derived"), report);
            for via in &derived.via {
                if !ctx
                    .resolver
                    .relation_exists(&edge.from, via, &edge.relation)
                {
                    push(
                        report,
                        codes::blocker::SPEC_DERIVED_TRACE_INVALID,
                        &format!("{at}.derived.via"),
                        format!("推导路径上的关系不存在：{} → {}", edge.from, via),
                        Some("修正 via 路径或移除 derived 标记".to_string()),
                    );
                }
            }
        }

        validate_nature(spec, edge.nature, &edge.from, &edge.to, &at, report);
    }

    for (index, node) in spec.nodes.iter().enumerate() {
        if !connected.contains(node.id.as_str()) {
            push(
                report,
                codes::warning::SPEC_NODE_WITHOUT_EDGE,
                &format!("nodes[{index}].id"),
                format!("孤立节点：{}", node.id),
                None,
            );
        }
    }
}

fn validate_endpoint_types(
    spec: &VizSpec,
    edge: &crate::visualization::spec::VizEdge,
    relation: &crate::visualization::registry::RelationDefinition,
    at: &str,
    report: &mut ValidationReport,
) {
    let find_kind = |id: &str| {
        spec.nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.kind.clone())
    };
    if let Some(kind) = find_kind(&edge.from) {
        if !relation.accepts_endpoint(&kind, true) {
            push(
                report,
                codes::blocker::SPEC_RELATION_KIND_MISMATCH,
                &format!("{at}.from"),
                format!(
                    "关系 {} 的起点类型为 {}，与节点类型 {} 不符",
                    relation.key, relation.source_type, kind
                ),
                None,
            );
        }
    }
    if let Some(kind) = find_kind(&edge.to) {
        if !relation.accepts_endpoint(&kind, false) {
            push(
                report,
                codes::blocker::SPEC_RELATION_KIND_MISMATCH,
                &format!("{at}.to"),
                format!(
                    "关系 {} 的终点类型为 {}，与节点类型 {} 不符",
                    relation.key, relation.target_type, kind
                ),
                None,
            );
        }
    }
}

/// 边的性质不得与其端点节点性质冲突：business 边不能挂在 explanatory 节点上。
fn validate_nature(
    spec: &VizSpec,
    edge_nature: Option<Nature>,
    from: &str,
    to: &str,
    at: &str,
    report: &mut ValidationReport,
) {
    let artifact_nature = spec.meta.nature;
    let effective = edge_nature.or(artifact_nature).unwrap_or(Nature::Business);
    if effective != Nature::Business {
        return;
    }
    for (field, id) in [("from", from), ("to", to)] {
        if let Some(node) = spec.nodes.iter().find(|n| n.id == id) {
            let node_nature = node.nature.or(artifact_nature).unwrap_or(Nature::Business);
            if node_nature == Nature::Explanatory {
                push(
                    report,
                    codes::blocker::SPEC_NATURE_MISMATCH,
                    &format!("{at}.{field}"),
                    format!("business 边不能建立在 explanatory 节点上：{id}"),
                    Some("把节点或边标记为 explanatory".to_string()),
                );
            }
        }
    }
}

fn validate_derived(derived: &DerivedTrace, at: &str, report: &mut ValidationReport) {
    if !derived.is_structurally_valid() {
        push(
            report,
            codes::blocker::SPEC_DERIVED_TRACE_INVALID,
            at,
            "间接推导标记必须为 direct=false 且 via 非空".to_string(),
            Some("修正 derived 标记或移除它".to_string()),
        );
    }
}

fn validate_view(spec: &VizSpec, report: &mut ValidationReport) {
    let node_ids: HashSet<&str> = spec.nodes.iter().map(|n| n.id.as_str()).collect();
    for (index, focus) in spec.view.focus.iter().enumerate() {
        if !node_ids.contains(focus.as_str()) {
            push(
                report,
                codes::warning::SPEC_FOCUS_UNMATCHED,
                &format!("view.focus[{index}]"),
                format!("焦点未命中任何节点：{focus}"),
                None,
            );
        }
    }
    if spec.view.lenses.is_empty() {
        push(
            report,
            codes::warning::SPEC_LENS_UNMATCHED,
            "view.lenses",
            "未定义任何透镜".to_string(),
            None,
        );
    }
    if let Some(hint) = &spec.view.layout_hint {
        if !LAYOUT_HINTS.contains(&hint.as_str()) {
            push(
                report,
                codes::warning::SPEC_LAYOUT_HINT_CONFLICT,
                "view.layout_hint",
                format!("未知布局意图：{hint}"),
                Some("使用已知布局意图或省略该字段".to_string()),
            );
        }
    }
}

/// 已知布局意图（渲染器可忽略，取值仅作意图声明）。
const LAYOUT_HINTS: &[&str] = &[
    "layered-lr",
    "layered-tb",
    "radial",
    "force",
    "grid",
    "swimlane",
];

fn validate_label(
    label: &str,
    at: &str,
    ctx: &ValidationContext<'_>,
    report: &mut ValidationReport,
) {
    let len = label.chars().count();
    if len > ctx.limits.max_label_len {
        push(
            report,
            codes::blocker::SPEC_LABEL_TOO_LONG,
            at,
            format!("标签长度 {len} 超出上限 {}", ctx.limits.max_label_len),
            Some("精简标签".to_string()),
        );
    }
    if label.chars().any(|c| c.is_control()) {
        push(
            report,
            codes::blocker::SPEC_LABEL_CONTROL_CHARS,
            at,
            "标签含控制字符".to_string(),
            Some("移除控制字符".to_string()),
        );
    }
}

/// 解析 `major.minor` 版本号。
fn parse_version(value: &str) -> Option<(u32, u32)> {
    let (major, minor) = value.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

fn push(report: &mut ValidationReport, code: &str, at: &str, message: String, fix: Option<String>) {
    report.problems.push(VizProblem {
        code: code.to_string(),
        at: at.to_string(),
        message,
        fix,
    });
}

/// 预算档位与节点类型的辅助（供应用层构造上下文）。
pub fn default_limits_for(budget: Budget) -> SpecLimits {
    let limits = SpecLimits::default();
    match budget {
        Budget::Strict => limits,
        Budget::Balanced => limits,
        Budget::Faithful => limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visualization::registry::{NodeKindDefinition, RelationDefinition, RelationStatus};
    use crate::visualization::source_ref::SourceRef;
    use crate::visualization::spec::{VizEdge, VizLens, VizMeta, VizNode, VizView};

    /// 全部对象可见的解析器（单测用）。
    struct AllVisible;

    impl ObjectResolver for AllVisible {
        fn exists_and_visible(&self, _d: &str, _t: &str, _i: &str) -> bool {
            true
        }
        fn relation_exists(&self, _f: &str, _t: &str, _r: &str) -> bool {
            true
        }
    }

    fn registry() -> RelationRegistry {
        RelationRegistry {
            version: "1".to_string(),
            relations: vec![RelationDefinition {
                key: "design.covers_requirement".to_string(),
                status: RelationStatus::Active,
                domain: "design".to_string(),
                owner_role: "design-owner".to_string(),
                source: "design.design_requirement".to_string(),
                source_type: "design_revision".to_string(),
                target_type: "requirement_revision".to_string(),
                direction: "directed".to_string(),
                semantics: "设计覆盖需求".to_string(),
                governance_level: Some("L2".to_string()),
                introduced_in: Some("1.0".to_string()),
                signoff_by: None,
                signoff_at: None,
                registry_version: None,
                semantic_version: None,
                activated_at: None,
                signoff_record: None,
                view_alias: false,
                reverse_view_of: None,
                synthetic: false,
                business_fact: true,
                type_key_source: None,
                ref_role_filter: None,
                cardinality: None,
            }],
            node_kinds: vec![
                NodeKindDefinition {
                    key: "requirement_revision".to_string(),
                    semantics: "需求修订".to_string(),
                    domain: "delivery".to_string(),
                    owner_role: "delivery-owner".to_string(),
                    status: RelationStatus::Active,
                },
                NodeKindDefinition {
                    key: "design_revision".to_string(),
                    semantics: "设计修订".to_string(),
                    domain: "design".to_string(),
                    owner_role: "design-owner".to_string(),
                    status: RelationStatus::Active,
                },
            ],
        }
    }

    fn node(id: &str, kind: &str) -> VizNode {
        VizNode {
            id: id.to_string(),
            label: format!("节点 {id}"),
            kind: kind.to_string(),
            status: Some("confirmed".to_string()),
            nature: None,
            source_ref: Some(SourceRef::new("delivery", "requirement", id)),
            derived: None,
        }
    }

    fn edge(id: &str, from: &str, to: &str) -> VizEdge {
        VizEdge {
            id: id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            relation: "design.covers_requirement".to_string(),
            label: None,
            nature: None,
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
                lenses: vec![VizLens {
                    name: "coverage".to_string(),
                    r#match: serde_json::json!({ "edge_type": "covers" }),
                }],
                layout_hint: Some("layered-lr".to_string()),
            },
            nodes: vec![
                node("R-042", "requirement_revision"),
                node("D-017", "design_revision"),
            ],
            // design.covers_requirement 的语义方向是 design_revision → requirement_revision
            edges: vec![edge("e1", "D-017", "R-042")],
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

    fn ctx<'a>(
        registry: &'a RelationRegistry,
        resolver: &'a dyn ObjectResolver,
    ) -> ValidationContext<'a> {
        ValidationContext {
            registry,
            resolver,
            limits: SpecLimits::default(),
            allowed_slots: &[],
            current_registry_version: None,
        }
    }

    fn codes_of(report: &ValidationReport) -> Vec<&str> {
        report.problems.iter().map(|p| p.code.as_str()).collect()
    }

    #[test]
    fn valid_spec_passes_without_blockers() {
        let registry = registry();
        let resolver = AllVisible;
        let report = validate(&spec(), &ctx(&registry, &resolver));
        assert!(!report.has_blockers(), "阻断问题：{:?}", codes_of(&report));
    }

    #[test]
    fn duplicate_node_id_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.nodes.push(node("R-042", "requirement_revision"));
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_DUPLICATE_NODE_ID));
        assert!(report.has_blockers());
    }

    #[test]
    fn unknown_edge_endpoint_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.edges.push(edge("e2", "R-042", "D-999"));
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_EDGE_ENDPOINT_UNKNOWN));
    }

    #[test]
    fn unregistered_relation_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.edges[0].relation = "task.implements_requirement".to_string();
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_RELATION_NOT_REGISTERED));
    }

    #[test]
    fn draft_relation_is_blocker() {
        let mut registry = registry();
        registry.relations[0].status = RelationStatus::Draft;
        let resolver = AllVisible;
        let report = validate(&spec(), &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_RELATION_NOT_APPROVED));
    }

    #[test]
    fn unregistered_node_kind_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.nodes[0].kind = "mystery".to_string();
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_NODE_KIND_NOT_REGISTERED));
    }

    #[test]
    fn relation_kind_mismatch_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        // 起点是 requirement_revision，但该关系要求 design_revision 起点
        s.edges[0].from = "R-042".to_string();
        s.edges[0].to = "D-017".to_string();
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_RELATION_KIND_MISMATCH));
    }

    #[test]
    fn budget_exceeded_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.meta.budget = Budget::Strict;
        for i in 0..40 {
            s.nodes
                .push(node(&format!("X-{i}"), "requirement_revision"));
        }
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_BUDGET_EXCEEDED));
    }

    #[test]
    fn long_label_and_control_chars_are_blockers() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.nodes[0].label = "x".repeat(500);
        s.nodes[1].label = "带\u{0007}控制符".to_string();
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_LABEL_TOO_LONG));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_LABEL_CONTROL_CHARS));
    }

    #[test]
    fn slot_not_in_allowlist_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let allowed = vec!["other-slot".to_string()];
        let mut c = ctx(&registry, &resolver);
        c.allowed_slots = &allowed;
        let report = validate(&spec(), &c);
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_SLOT_NOT_ALLOWED));
    }

    #[test]
    fn incompatible_version_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.spec_version = "2.0".to_string();
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_VERSION_UNSUPPORTED));
    }

    #[test]
    fn newer_minor_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.spec_version = "1.9".to_string();
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_VERSION_UNSUPPORTED));
    }

    #[test]
    fn nature_mismatch_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.nodes[0].nature = Some(Nature::Explanatory);
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_NATURE_MISMATCH));
    }

    #[test]
    fn invalid_derived_trace_is_blocker() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.edges[0].derived = Some(DerivedTrace {
            direct: true,
            via: vec![],
            semantic_confidence: "structurally_supported".to_string(),
        });
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(codes_of(&report).contains(&codes::blocker::SPEC_DERIVED_TRACE_INVALID));
    }

    #[test]
    fn warnings_do_not_block() {
        let registry = registry();
        let resolver = AllVisible;
        let mut s = spec();
        s.view.focus = vec!["MISSING".to_string()];
        s.view.layout_hint = Some("unknown-hint".to_string());
        let report = validate(&s, &ctx(&registry, &resolver));
        assert!(!report.has_blockers(), "这些问题不该阻断");
        assert!(codes_of(&report).contains(&codes::warning::SPEC_FOCUS_UNMATCHED));
        assert!(codes_of(&report).contains(&codes::warning::SPEC_LAYOUT_HINT_CONFLICT));
    }

    #[test]
    fn severity_is_derived_from_code_prefix() {
        let blocker = VizProblem {
            code: codes::blocker::SPEC_BUDGET_EXCEEDED.to_string(),
            at: "meta".to_string(),
            message: "x".to_string(),
            fix: None,
        };
        assert_eq!(blocker.severity(), Severity::Blocker);
        let warning = VizProblem {
            code: codes::warning::SPEC_DANGLING_EDGE.to_string(),
            at: "edges[0]".to_string(),
            message: "x".to_string(),
            fix: None,
        };
        assert_eq!(warning.severity(), Severity::Warning);
    }

    #[test]
    fn registry_helper_methods_exist() {
        let r = registry();
        assert!(r.has_relation("design.covers_requirement"));
        assert!(r.is_relation_projectable("design.covers_requirement"));
        assert!(r.node_kind("design_revision").is_some());
    }
}
