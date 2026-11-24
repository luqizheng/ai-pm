//! VizSpec → SeeCode spec 编译器（Adapter B 的**唯一**耦合点）。
//!
//! 依据 docs/visualization/ai-pm-viz-02-seecode-research-v1.md §6：
//! **spec 是编译目标，不是数据模型**——SeeCode 的字段（row/col/budget/kind）
//! 只存在于本模块内部，业务领域与 VizSpec 不得感知（`viz-spec-lint` 守卫）。
//!
//! ## SeeCode 的硬约束（来自 `tools/seecode/schemas/graph.schema.json`）
//!
//! | 约束 | 本模块处理 |
//! |---|---|
//! | `node.id` 必须匹配 `^[A-Za-z_][A-Za-z0-9_.-]*$` | UUID 以数字开头会非法 → 加 `n_` 前缀并替换非法字符 |
//! | `node.label` ≤40 字符 | 按字符截断（不切多字节边界） |
//! | `node.kind` 受控八值 | VizSpec kind → SeeCode kind 映射表 |
//! | 预算 ≤9 节点 / 12 边（strict），faithful ≤24 | 超限时按焦点邻近做 top-K 裁剪并标记 warning |
//! | 边 label ≤28 字符 | 关系键转短标签 |
//!
//! ## 无损原则
//!
//! 裁剪必须**显式**：返回 `CompiledSpec.warnings`，由渲染产物页脚或日志呈现
//! （「显示 N/M 节点」），不得静默丢弃。

use serde_json::json;

use pm_domain::visualization::{SourceRef, VizNode, VizSpec};

/// 编译结果。
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSpec {
    /// SeeCode spec（JSON）。
    pub spec: serde_json::Value,
    /// 裁剪/降级告警（空 = 无损编译）。
    pub warnings: Vec<String>,
    /// 实际编入的节点数。
    pub node_count: usize,
    /// 实际编入的边数。
    pub edge_count: usize,
}

/// 编译失败（超出渲染器能力，按契约第 6 项应由调用方走 Rejected 路径）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompileError {
    /// 无节点可渲染。
    #[error("spec has no renderable nodes")]
    Empty,
    /// 节点数超出渲染器宽容上限（faithful）。
    #[error("spec has {count} nodes, exceeding renderer limit {limit}")]
    TooManyNodes { count: usize, limit: usize },
}

/// SeeCode 宽容上限（`budget: faithful`）。
const MAX_NODES: usize = 24;
/// SeeCode 节点 label 上限。
const MAX_LABEL_CHARS: usize = 40;
/// SeeCode 边 label 上限。
const MAX_EDGE_LABEL_CHARS: usize = 28;

/// VizSpec 节点类型 → SeeCode 视觉 kind（受控八值）。
fn visual_kind(node_kind: &str) -> &'static str {
    match node_kind {
        // 需求是图的输入源
        "requirement_revision" | "requirement" => "input",
        // 设计与 ADR 是后端构件
        "design_revision" | "design" => "backend",
        "adr_revision" | "adr" => "security",
        // 任务是可选项（前置未完成即阻塞）
        "task" => "optional",
        // 知识/记忆/原始资料是存储
        "memory" | "source" | "knowledge" | "document" => "store",
        // 认知对象是外部语义
        "cognition_object" | "pc_object" | "module" | "service" | "repository" => "external",
        _ => "backend",
    }
}

/// VizSpec 状态 → SeeCode status（ok/risk/legacy/retire）。
fn visual_status(status: Option<&str>) -> Option<&'static str> {
    match status? {
        "completed" | "approved" | "published" | "confirmed" | "active" => Some("ok"),
        "blocked" | "failed" | "awaiting_decision" | "changes_requested" => Some("risk"),
        "superseded" | "archived" | "withdrawn" => Some("legacy"),
        "cancelled" | "rejected" | "retired" => Some("retire"),
        _ => None,
    }
}

/// 关系键 → 边短标签（≤28 字符）。
fn edge_label(relation: &str) -> String {
    let short = match relation {
        "design.covers_requirement" => "covers",
        "requirement.refines_design" => "refines",
        "design.decided_by_adr" => "decided by",
        "adr.covers_requirement" => "covers",
        "task.implements_requirement" => "implements",
        "task.depends_on_task" => "depends on",
        "pc.related" => "related",
        "kb.memory_supported_by_source" => "based on",
        "derived_trace" => "derived",
        other => other,
    };
    truncate_chars(short, MAX_EDGE_LABEL_CHARS)
}

/// 关系键 → SeeCode 边 kind。
fn edge_kind(relation: &str) -> &'static str {
    match relation {
        "task.depends_on_task" => "depends",
        "design.covers_requirement" | "task.implements_requirement" => "primary",
        "derived_trace" => "muted",
        "adr.covers_requirement" | "design.decided_by_adr" => "assoc",
        "kb.memory_supported_by_source" => "link",
        _ => "default",
    }
}

/// 源码 → SeeCode 节点 id（必须 `^[A-Za-z_][A-Za-z0-9_.-]*$`）。
///
/// UUID 常以数字开头（如 `1a2b…`），直接使用会违反 schema——统一加 `n_` 前缀。
fn node_id(source: &str) -> String {
    let sanitized: String = source
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitize_multi_underscore(&sanitized);
    format!("n_{sanitized}")
}

fn sanitize_multi_underscore(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut prev_underscore = false;
    for c in value.chars() {
        if c == '_' {
            if !prev_underscore {
                out.push(c);
            }
            prev_underscore = true;
        } else {
            out.push(c);
            prev_underscore = false;
        }
    }
    out
}

/// 按**字符**（不是字节）截断，保证中文不被切坏。
fn truncate_chars(value: &str, limit: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= limit {
        return value.to_string();
    }
    let mut truncated: String = chars[..limit.saturating_sub(1)].iter().collect();
    truncated.push('…');
    truncated
}

/// 节点在分层布局中的列（按领域深度）。
fn layout_col(node: &VizNode, kind: &str) -> u32 {
    match kind {
        "input" => 0,
        "backend" => 1,
        "security" => 2,
        "optional" => 3,
        "store" => 4,
        "external" => 1,
        _ => 2,
    }
    .min(40)
    .max({
        let _ = node;
        0
    })
}

/// 编译：VizSpec → SeeCode spec。
///
/// 超预算时按「焦点节点优先 + 度数优先」裁剪，并在 `warnings` 中显式记录。
pub fn compile(spec: &VizSpec) -> Result<CompiledSpec, CompileError> {
    if spec.nodes.is_empty() {
        return Err(CompileError::Empty);
    }
    if spec.nodes.len() > MAX_NODES * 4 {
        // 远超能力：直接拒绝（契约第 6 项要求显式拒绝而非产出残缺图）
        return Err(CompileError::TooManyNodes {
            count: spec.nodes.len(),
            limit: MAX_NODES,
        });
    }

    let mut warnings: Vec<String> = Vec::new();

    // ── 选点：焦点优先，其次按度数（与焦点间接相关者先入）
    let degree = |id: &str| {
        spec.edges
            .iter()
            .filter(|e| e.from == id || e.to == id)
            .count()
    };
    let mut selected: Vec<&VizNode> = spec.nodes.iter().collect();
    if selected.len() > MAX_NODES {
        let focus: Vec<&str> = spec.view.focus.iter().map(String::as_str).collect();
        selected.sort_by_key(|n| {
            let is_focus = focus.contains(&n.id.as_str());
            // 焦点最优先（false 排后 → 用 !is_focus 作主键），其次度数高者优先
            (!is_focus, std::cmp::Reverse(degree(&n.id)))
        });
        let kept = selected.len();
        selected.truncate(MAX_NODES);
        warnings.push(format!(
            "节点超渲染器上限：显示 {} / {} 个节点（完整关系请用在线视图）",
            MAX_NODES, kept
        ));
    }

    let kept_ids: std::collections::HashSet<&str> =
        selected.iter().map(|n| n.id.as_str()).collect();

    // ── 节点
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut col_cursor: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for node in &selected {
        let kind = visual_kind(&node.kind);
        let col = layout_col(node, kind);
        let row = {
            let cursor = col_cursor.entry(col).or_insert(0);
            let row = (*cursor).min(40);
            *cursor += 1;
            row
        };
        let mut entry = json!({
            "id": node_id(&node.id),
            "label": truncate_chars(&node.label, MAX_LABEL_CHARS),
            "kind": kind,
            "row": row,
            "col": col,
        });
        if spec.view.focus.contains(&node.id) {
            entry["focal"] = json!(true);
        }
        if let Some(status) = visual_status(node.status.as_deref()) {
            entry["status"] = json!(status);
        }
        // 溯源进 sub（file:line 风格与 SeeCode evidence 理念一致）
        if let Some(SourceRef {
            domain, r#type, id, ..
        }) = &node.source_ref
        {
            entry["sub"] = json!(truncate_chars(&format!("{domain}/{type}#{id}"), 48));
        }
        nodes.push(entry);
    }

    // ── 边（两端都必须入选，否则整边丢弃并计数）
    let mut edges: Vec<serde_json::Value> = Vec::new();
    let mut dropped_edges = 0usize;
    for edge in &spec.edges {
        if !kept_ids.contains(edge.from.as_str()) || !kept_ids.contains(edge.to.as_str()) {
            dropped_edges += 1;
            continue;
        }
        let label = edge_label(&edge.relation);
        edges.push(json!([
            node_id(&edge.from),
            node_id(&edge.to),
            label,
            edge_kind(&edge.relation)
        ]));
    }
    if dropped_edges > 0 {
        warnings.push(format!("因节点裁剪丢弃 {dropped_edges} 条边（端点未入选）"));
    }

    // ── envelope
    let title = spec
        .meta
        .title
        .clone()
        .unwrap_or_else(|| "AI-PM 关系图".to_string());
    let balanced = nodes.len() > 9 || edges.len() > 12;
    let mut compiled = json!({
        "type": "architecture",
        "title": truncate_chars(&title, 90),
        "subtitle": truncate_chars(&format!("{} 个节点 · {} 条关系", nodes.len(), edges.len()), 160),
        "motion": "none",
        "budget": if balanced { "faithful" } else { "strict" },
        "legend": true,
        "nodes": nodes,
        "edges": edges,
    });
    if let Some(lang) = &spec.meta.lang {
        compiled["lang"] = json!(lang);
    }

    Ok(CompiledSpec {
        spec: compiled,
        warnings,
        node_count: selected.len(),
        edge_count: edges.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_domain::visualization::{Budget, Nature, VizEdge, VizMeta, VizView, SPEC_VERSION};

    fn node(id: &str, kind: &str, label: &str) -> VizNode {
        VizNode {
            id: id.to_string(),
            label: label.to_string(),
            kind: kind.to_string(),
            status: None,
            nature: Some(Nature::Business),
            source_ref: Some(SourceRef::new("delivery", "requirement", id)),
            derived: None,
        }
    }

    fn spec_with(nodes: Vec<VizNode>, edges: Vec<VizEdge>) -> VizSpec {
        VizSpec {
            spec_version: SPEC_VERSION.to_string(),
            slot_key: "s".to_string(),
            view: VizView {
                kind: "trace-graph".to_string(),
                focus: vec![],
                lenses: vec![],
                layout_hint: None,
            },
            nodes,
            edges,
            meta: VizMeta {
                title: Some("测试图".to_string()),
                lang: Some("zh-CN".to_string()),
                budget: Budget::Balanced,
                nature: Some(Nature::Business),
                registry_version: None,
                provenance: None,
            },
        }
    }

    fn edge(from: &str, to: &str, relation: &str) -> VizEdge {
        VizEdge {
            id: format!("{from}-{to}"),
            from: from.to_string(),
            to: to.to_string(),
            relation: relation.to_string(),
            label: None,
            nature: None,
            derived: None,
        }
    }

    #[test]
    fn uuid_like_ids_are_prefixed_and_sanitized() {
        // UUID 以数字开头 → 必须加 n_ 前缀才符合 SeeCode 的 id pattern
        let spec = spec_with(
            vec![node("1a2b3c4d-0000-0000-0000-000000000000", "task", "任务")],
            vec![],
        );
        let compiled = compile(&spec).expect("编译");
        let id = compiled.spec["nodes"][0]["id"].as_str().unwrap();
        assert!(id.starts_with("n_"), "id 应有 n_ 前缀：{id}");
        assert!(
            id.chars()
                .next()
                .map(|c| c.is_ascii_alphabetic() || c == '_')
                .unwrap_or(false),
            "id 必须以字母或下划线开头"
        );
    }

    #[test]
    fn long_chinese_label_is_truncated_by_chars() {
        let long = "这是一个非常长的中文标签".repeat(10);
        let spec = spec_with(vec![node("n1", "task", &long)], vec![]);
        let compiled = compile(&spec).expect("编译");
        let label = compiled.spec["nodes"][0]["label"].as_str().unwrap();
        assert!(
            label.chars().count() <= MAX_LABEL_CHARS,
            "label 应 ≤{MAX_LABEL_CHARS} 字符，实际 {}",
            label.chars().count()
        );
        assert!(label.ends_with('…'), "截断应带省略号");
    }

    #[test]
    fn kinds_map_to_controlled_vocabulary() {
        let spec = spec_with(
            vec![
                node("r1", "requirement_revision", "需求"),
                node("d1", "design_revision", "设计"),
                node("a1", "adr_revision", "ADR"),
                node("t1", "task", "任务"),
                node("m1", "memory", "记忆"),
            ],
            vec![],
        );
        let compiled = compile(&spec).expect("编译");
        let allowed = [
            "backend", "focal", "store", "external", "input", "optional", "security", "muted",
        ];
        for entry in compiled.spec["nodes"].as_array().unwrap() {
            let kind = entry["kind"].as_str().unwrap();
            assert!(allowed.contains(&kind), "kind 必须受控：{kind}");
        }
        assert_eq!(compiled.spec["nodes"][0]["kind"], "input");
        assert_eq!(compiled.spec["nodes"][3]["kind"], "optional");
    }

    #[test]
    fn edges_reference_only_kept_nodes() {
        let spec = spec_with(
            vec![node("n1", "task", "A"), node("n2", "task", "B")],
            vec![edge("n1", "n2", "task.depends_on_task")],
        );
        let compiled = compile(&spec).expect("编译");
        assert_eq!(compiled.edge_count, 1);
        let e = &compiled.spec["edges"][0];
        assert_eq!(e[0], "n_n1");
        assert_eq!(e[1], "n_n2");
        assert_eq!(e[2], "depends on");
        assert_eq!(e[3], "depends");
    }

    #[test]
    fn oversize_spec_is_truncated_with_warning() {
        // 30 个节点 → 裁到 24，并显式告警（不得静默丢弃）
        let nodes: Vec<VizNode> = (0..30)
            .map(|i| node(&format!("n{i}"), "task", &format!("任务{i}")))
            .collect();
        let spec = spec_with(nodes, vec![]);
        let compiled = compile(&spec).expect("编译");
        assert_eq!(compiled.node_count, MAX_NODES);
        assert!(!compiled.warnings.is_empty(), "裁剪必须告警");
        assert!(
            compiled.warnings[0].contains("显示 24 / 30"),
            "{:?}",
            compiled.warnings
        );
    }

    #[test]
    fn far_oversize_spec_is_rejected() {
        let nodes: Vec<VizNode> = (0..200)
            .map(|i| node(&format!("n{i}"), "task", "t"))
            .collect();
        let spec = spec_with(nodes, vec![]);
        assert!(matches!(
            compile(&spec),
            Err(CompileError::TooManyNodes { .. })
        ));
    }

    #[test]
    fn empty_spec_is_rejected() {
        assert_eq!(
            compile(&spec_with(vec![], vec![])),
            Err(CompileError::Empty)
        );
    }

    #[test]
    fn focus_node_is_kept_first_when_truncating() {
        let mut nodes: Vec<VizNode> = (0..30)
            .map(|i| node(&format!("n{i}"), "task", &format!("t{i}")))
            .collect();
        // 最后一个节点设为焦点——裁剪后必须保留
        let focus_id = nodes[29].id.clone();
        nodes[29].label = "焦点节点".to_string();
        let mut spec = spec_with(nodes, vec![]);
        spec.view.focus = vec![focus_id.clone()];
        let compiled = compile(&spec).expect("编译");
        let labels: Vec<&str> = compiled.spec["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["label"].as_str().unwrap())
            .collect();
        assert!(labels.contains(&"焦点节点"), "焦点节点必须保留：{labels:?}");
    }

    #[test]
    fn status_maps_to_controlled_values() {
        let mut blocked = node("t1", "task", "阻塞任务");
        blocked.status = Some("blocked".to_string());
        let mut done = node("t2", "task", "完成任务");
        done.status = Some("completed".to_string());
        let spec = spec_with(vec![blocked, done], vec![]);
        let compiled = compile(&spec).expect("编译");
        assert_eq!(compiled.spec["nodes"][0]["status"], "risk");
        assert_eq!(compiled.spec["nodes"][1]["status"], "ok");
    }

    #[test]
    fn source_ref_lands_in_sub_for_traceability() {
        let spec = spec_with(vec![node("n1", "task", "任务")], vec![]);
        let compiled = compile(&spec).expect("编译");
        let sub = compiled.spec["nodes"][0]["sub"].as_str().unwrap();
        assert!(
            sub.contains("delivery/requirement"),
            "溯源应在 sub 中：{sub}"
        );
    }

    #[test]
    fn spec_contains_no_renderer_fields_leak() {
        // 反向守卫：VizSpec 的字段名不得出现在编译产物里（除已映射者）
        let spec = spec_with(vec![node("n1", "task", "任务")], vec![]);
        let compiled = compile(&spec).expect("编译");
        let text = serde_json::to_string(&compiled.spec).unwrap();
        assert!(!text.contains("source_ref"), "source_ref 不应原样外泄");
        assert!(!text.contains("slot_key"), "slot_key 不应进渲染 spec");
        assert!(
            !text.contains("registry_version"),
            "registry_version 不应进渲染 spec"
        );
    }
}
