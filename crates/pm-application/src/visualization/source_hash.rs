//! source_hash：图形输入指纹（stale 判定的唯一依据）。
//!
//! 依据：12 号 V0-1 设计 §6；ADR-VIZ-006（stale 检测方式在 V0-3 确定，
//! 不自动重渲染）。
//!
//! 编码与算法口径**对齐仓库既有惯例**：
//! - 排序先例：`review/anchor.rs:93-114`（按 id 排序后哈希，锚定与入库顺序无关）；
//! - 编码：`pm_write/support.rs:3-11` 的小写 hex sha256，**不加 `sha256:` 前缀**。
//!
//! 输入五要素（ADR-VIZ-006）：
//! ① 来源业务对象　② 来源 revision　③ Spec 核心内容　④ 参与推导的关系版本
//! ⑤ 影响图形解释的配置版本。
//!
//! 排序要求：**与输入顺序无关**——同一批对象换个顺序组装，source_hash 必须相同。

use pm_domain::visualization::spec::VizSpec;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// 参与哈希的来源引用（规范化形态）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SourceFingerprint {
    domain: String,
    r#type: String,
    id: String,
    revision_id: String,
    content_hash: String,
}

/// 参与哈希的推导关系版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DerivedFingerprint {
    relation: String,
    via: Vec<String>,
}

/// source_hash 的载荷（字段顺序即序列化顺序，必须稳定）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SourceHashPayload {
    sources: Vec<SourceFingerprint>,
    derived: Vec<DerivedFingerprint>,
    spec_core: SpecCore,
    config: ConfigFingerprint,
}

/// Spec 核心内容（view / nodes / edges —— 不含 meta 里的易变字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SpecCore {
    view: serde_json::Value,
    nodes: Vec<serde_json::Value>,
    edges: Vec<serde_json::Value>,
}

/// 影响图形解释的配置版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ConfigFingerprint {
    budget: String,
    registry_version: String,
}

/// 计算 source_hash：小写 hex sha256。
///
/// 结果与 Spec 的组装顺序**无关**（来源与推导项均先排序）。
pub fn source_hash(spec: &VizSpec) -> String {
    let payload = build_payload(spec);
    let bytes = serde_json::to_vec(&payload).expect("source hash payload only contains primitives");
    hex(&Sha256::digest(&bytes))
}

/// 构造哈希载荷（便于测试直接断言载荷内容）。
fn build_payload(spec: &VizSpec) -> SourceHashPayload {
    let mut sources: Vec<SourceFingerprint> = spec
        .nodes
        .iter()
        .filter_map(|node| {
            node.source_ref.as_ref().map(|source_ref| SourceFingerprint {
                domain: source_ref.domain.clone(),
                r#type: source_ref.r#type.clone(),
                id: source_ref.id.clone(),
                revision_id: source_ref.revision_id.clone().unwrap_or_default(),
                content_hash: source_ref.content_hash.clone().unwrap_or_default(),
            })
        })
        .collect();
    sources.sort_by(|a, b| {
        (a.domain.as_str(), a.r#type.as_str(), a.id.as_str(), a.revision_id.as_str()).cmp(&(
            b.domain.as_str(),
            b.r#type.as_str(),
            b.id.as_str(),
            b.revision_id.as_str(),
        ))
    });
    sources.dedup_by(|a, b| {
        a.domain == b.domain && a.r#type == b.r#type && a.id == b.id && a.revision_id == b.revision_id
    });

    let mut derived: Vec<DerivedFingerprint> = spec
        .edges
        .iter()
        .filter_map(|edge| {
            edge.derived.as_ref().map(|d| DerivedFingerprint {
                relation: edge.relation.clone(),
                via: d.via.clone(),
            })
        })
        .collect();
    derived.sort_by(|a, b| (a.relation.as_str(), &a.via).cmp(&(b.relation.as_str(), &b.via)));

    SourceHashPayload {
        sources,
        derived,
        spec_core: SpecCore {
            view: serde_json::to_value(&spec.view).expect("view serializable"),
            // 节点/边按稳定键排序：Spec 不承载布局顺序（布局由渲染器决定），
            // 因此哈希必须反映「图的内容」而非「数组顺序」。
            nodes: sorted_nodes(spec),
            edges: sorted_edges(spec),
        },
        config: ConfigFingerprint {
            budget: spec.meta.budget.as_str().to_string(),
            registry_version: spec.meta.registry_version.clone().unwrap_or_default(),
        },
    }
}

/// 节点按 id 排序后的 JSON 列表。
fn sorted_nodes(spec: &VizSpec) -> Vec<serde_json::Value> {
    let mut indexed: Vec<(&str, serde_json::Value)> = spec
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.as_str(),
                serde_json::to_value(node).expect("node serializable"),
            )
        })
        .collect();
    indexed.sort_by(|a, b| a.0.cmp(b.0));
    indexed.into_iter().map(|(_, value)| value).collect()
}

/// 边按 (from, to, relation, id) 排序后的 JSON 列表。
fn sorted_edges(spec: &VizSpec) -> Vec<serde_json::Value> {
    let mut indexed: Vec<((&str, &str, &str, &str), serde_json::Value)> = spec
        .edges
        .iter()
        .map(|edge| {
            (
                (
                    edge.from.as_str(),
                    edge.to.as_str(),
                    edge.relation.as_str(),
                    edge.id.as_str(),
                ),
                serde_json::to_value(edge).expect("edge serializable"),
            )
        })
        .collect();
    indexed.sort_by(|a, b| a.0.cmp(&b.0));
    indexed.into_iter().map(|(_, value)| value).collect()
}

/// 小写 hex 编码（对齐 `pm_write::hex_sha256`，无 hex crate 依赖）。
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_domain::visualization::spec::{
        Budget, VizEdge, VizMeta, VizNode, VizSpec, VizView, SPEC_VERSION,
    };
    use pm_domain::visualization::source_ref::{DerivedTrace, SourceRef};

    fn node(id: &str, revision: &str) -> VizNode {
        VizNode {
            id: id.to_string(),
            label: format!("节点 {id}"),
            kind: "requirement_revision".to_string(),
            status: None,
            nature: None,
            source_ref: Some(
                SourceRef::new("delivery", "requirement", id)
                    .with_revision(revision)
                    .with_content_hash("hash-1"),
            ),
            derived: None,
        }
    }

    fn spec() -> VizSpec {
        VizSpec {
            spec_version: SPEC_VERSION.to_string(),
            slot_key: "req-trace:R-042".to_string(),
            view: VizView {
                kind: "trace-graph".to_string(),
                focus: vec![],
                lenses: vec![],
                layout_hint: None,
            },
            nodes: vec![node("R-042", "rev-12"), node("R-043", "rev-3")],
            edges: vec![VizEdge {
                id: "e1".to_string(),
                from: "R-042".to_string(),
                to: "R-043".to_string(),
                relation: "design.covers_requirement".to_string(),
                label: None,
                nature: None,
                derived: Some(DerivedTrace::indirect(vec!["R-043".to_string()])),
            }],
            meta: VizMeta {
                title: None,
                lang: None,
                budget: Budget::Balanced,
                nature: None,
                registry_version: Some("1".to_string()),
                provenance: None,
            },
        }
    }

    #[test]
    fn hash_is_64_lowercase_hex() {
        let hash = source_hash(&spec());
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(source_hash(&spec()), source_hash(&spec()));
    }

    #[test]
    fn hash_is_independent_of_node_order() {
        let mut s = spec();
        s.nodes.reverse();
        assert_eq!(source_hash(&spec()), source_hash(&s));
    }

    #[test]
    fn hash_is_independent_of_edge_order() {
        let mut s = spec();
        s.edges.push(VizEdge {
            id: "e2".to_string(),
            from: "R-043".to_string(),
            to: "R-042".to_string(),
            relation: "task.implements_requirement".to_string(),
            label: None,
            nature: None,
            derived: None,
        });
        let before = source_hash(&s);
        s.edges.reverse();
        assert_eq!(before, source_hash(&s), "边顺序不得影响哈希");
    }

    #[test]
    fn hash_changes_when_revision_changes() {
        let base = source_hash(&spec());
        let mut s = spec();
        s.nodes[0].source_ref.as_mut().expect("source_ref").revision_id = Some("rev-13".to_string());
        assert_ne!(base, source_hash(&s));
    }

    #[test]
    fn hash_changes_when_content_hash_changes() {
        let base = source_hash(&spec());
        let mut s = spec();
        s.nodes[0].source_ref.as_mut().expect("source_ref").content_hash = Some("hash-2".to_string());
        assert_ne!(base, source_hash(&s));
    }

    #[test]
    fn hash_changes_when_registry_version_changes() {
        let base = source_hash(&spec());
        let mut s = spec();
        s.meta.registry_version = Some("2".to_string());
        assert_ne!(base, source_hash(&s));
    }

    #[test]
    fn hash_changes_when_budget_changes() {
        let base = source_hash(&spec());
        let mut s = spec();
        s.meta.budget = Budget::Strict;
        assert_ne!(base, source_hash(&s));
    }

    #[test]
    fn hash_changes_when_edge_relation_changes() {
        let base = source_hash(&spec());
        let mut s = spec();
        s.edges[0].relation = "task.implements_requirement".to_string();
        assert_ne!(base, source_hash(&s));
    }

    #[test]
    fn payload_dedups_identical_sources() {
        let mut s = spec();
        s.nodes.push(node("R-042", "rev-12"));
        let payload = build_payload(&s);
        assert_eq!(payload.sources.len(), 2, "重复来源应去重");
    }
}
