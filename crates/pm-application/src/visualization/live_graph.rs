//! Live 图组装（V0-3 通道接线）。
//!
//! 数据流（14 号 §3.2）：repo 取边 → 组装 VizSpec → **投影层裁剪**（权限 + 计数
//! 重算 + 一致性标记）→ 返回。投影层已实现（`projection.rs`），本模块只做组装。
//!
//! 两种视图：
//! - `trace-graph`：需求 ← 设计 ← 任务的追踪关系；
//! - `dag`：Task 依赖图（方向约定见 [`TaskDependencyRow`] 注释）。
//!
//! 溯源：Live 图的节点来源引用由边端点推导（domain/type 由关系键决定）；
//! revision 取绑定修订（14 号 §5.4：图内不出现"同一对象两个版本"）。

use uuid::Uuid;

use pm_domain::visualization::{
    Budget, Nature, RelationRegistry, SourceRef, VizEdge, VizMeta, VizNode, VizSpec, VizView,
    SPEC_VERSION,
};

use crate::repo::viz_read::{TaskDependencyRow, TraceEdgeRow};

/// 单次 Live 查询的边数上限（防大图拖垮响应；超限由调用方标记 truncated）。
pub const LIVE_EDGE_LIMIT: i64 = 500;

/// 组装追踪图 Spec。
///
/// 节点：边两端的对象（按 id 去重）；边：每条关系一行。
/// `source_ref` 用端点类型 + 绑定修订——溯源可达。
pub fn build_trace_spec(
    project_id: Uuid,
    registry: &RelationRegistry,
    rows: &[TraceEdgeRow],
    truncated: bool,
) -> VizSpec {
    let _ = project_id;
    let mut nodes: Vec<VizNode> = Vec::new();
    let mut edges: Vec<VizEdge> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    let push_node = |nodes: &mut Vec<VizNode>, seen: &mut Vec<String>, id: Uuid, title: &str, revision: Uuid, domain: &str, kind: &str| {
        let key = id.to_string();
        if seen.contains(&key) {
            return;
        }
        seen.push(key.clone());
        nodes.push(VizNode {
            id: key,
            label: if title.is_empty() {
                "（无标题）".to_string()
            } else {
                title.to_string()
            },
            kind: kind.to_string(),
            status: None,
            nature: Some(Nature::Business),
            source_ref: Some(
                SourceRef::new(domain, kind, id.to_string())
                    .with_revision(revision.to_string()),
            ),
            derived: None,
        });
    };

    for row in rows {
        // **注册表闸**：关系未 active 即不可投影（专家裁决：未会签 ≠ 默认允许）。
        // 跳过而不是报错——Live 图是读投影，未启用的关系静默不出边，
        // 由 `viz-registry-lint --readiness` 报告待会签条目。
        if !registry.is_relation_projectable(&row.relation) {
            continue;
        }
        // 关系键决定两端域/类型
        let (from_domain, from_kind, to_domain, to_kind) = match row.relation.as_str() {
            "design.covers_requirement" => (
                "design",
                "design_revision",
                "delivery",
                "requirement_revision",
            ),
            _ => ("delivery", "task", "delivery", "requirement_revision"),
        };
        push_node(
            &mut nodes,
            &mut seen,
            row.from_id,
            &row.from_title,
            row.from_revision_id,
            from_domain,
            from_kind,
        );
        push_node(
            &mut nodes,
            &mut seen,
            row.to_id,
            &row.to_title,
            row.to_revision_id,
            to_domain,
            to_kind,
        );
        edges.push(VizEdge {
            id: format!("{}:{}:{}", row.relation, row.from_id, row.to_id),
            from: row.from_id.to_string(),
            to: row.to_id.to_string(),
            relation: row.relation.clone(),
            label: None,
            nature: Some(Nature::Business),
            derived: None,
        });
    }

    VizSpec {
        spec_version: SPEC_VERSION.to_string(),
        slot_key: "live:trace-graph".to_string(),
        view: VizView {
            kind: "trace-graph".to_string(),
            focus: vec![],
            lenses: vec![],
            layout_hint: Some("layered-lr".to_string()),
        },
        nodes,
        edges,
        meta: VizMeta {
            title: Some("需求·设计·任务追踪".to_string()),
            lang: Some("zh-CN".to_string()),
            budget: if truncated {
                Budget::Balanced
            } else {
                Budget::Faithful
            },
            nature: Some(Nature::Business),
            registry_version: None,
            provenance: None,
        },
    }
}

/// 组装 Task 依赖图 Spec。
///
/// **方向**：节点/边保持库内语义 `task_id`（依赖方）→ `depends_on_task_id`（前置方）。
/// DAG 视觉可反向展示（`prerequisite → dependent`），但那是 presentation transform，
/// 不在此层反转（17 号 R-05 / V-11）。
pub fn build_task_dag_spec(
    project_id: Uuid,
    registry: &RelationRegistry,
    rows: &[TaskDependencyRow],
    truncated: bool,
) -> VizSpec {
    let _ = project_id;
    let mut nodes: Vec<VizNode> = Vec::new();
    let mut edges: Vec<VizEdge> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    let push_node = |nodes: &mut Vec<VizNode>, seen: &mut Vec<String>, id: Uuid, title: &str, status: &str| {
        let key = id.to_string();
        if seen.contains(&key) {
            return;
        }
        seen.push(key.clone());
        nodes.push(VizNode {
            id: key,
            label: if title.is_empty() {
                "（无标题）".to_string()
            } else {
                title.to_string()
            },
            kind: "task".to_string(),
            status: if status.is_empty() {
                None
            } else {
                Some(status.to_string())
            },
            nature: Some(Nature::Business),
            // task 无内容版本——source_ref 不带 revision_id（不伪造内容版本，
            // 0019 迁移头注的同类纪律）
            source_ref: Some(SourceRef::new("delivery", "task", id.to_string())),
            derived: None,
        });
    };

    for row in rows {
        // 注册表闸（同上）：`task.depends_on_task` 未 active 则不出边
        if !registry.is_relation_projectable("task.depends_on_task") {
            continue;
        }
        push_node(
            &mut nodes,
            &mut seen,
            row.task_id,
            &row.task_title,
            &row.task_status,
        );
        push_node(
            &mut nodes,
            &mut seen,
            row.depends_on_task_id,
            &row.depends_on_title,
            &row.depends_on_status,
        );
        edges.push(VizEdge {
            id: format!("dep:{}:{}", row.task_id, row.depends_on_task_id),
            from: row.task_id.to_string(),
            to: row.depends_on_task_id.to_string(),
            relation: "task.depends_on_task".to_string(),
            label: None,
            nature: Some(Nature::Business),
            derived: None,
        });
    }

    VizSpec {
        spec_version: SPEC_VERSION.to_string(),
        slot_key: "live:task-dag".to_string(),
        view: VizView {
            kind: "dag".to_string(),
            focus: vec![],
            lenses: vec![],
            layout_hint: Some("layered-tb".to_string()),
        },
        nodes,
        edges,
        meta: VizMeta {
            title: Some("任务依赖图".to_string()),
            lang: Some("zh-CN".to_string()),
            budget: if truncated {
                Budget::Balanced
            } else {
                Budget::Faithful
            },
            nature: Some(Nature::Business),
            registry_version: None,
            provenance: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_domain::visualization::{RelationDefinition, RelationStatus};

    /// 测试注册表：三条关系全 active（含 G-1 审计字段）。
    fn active_registry() -> RelationRegistry {
        let active = |key: &str, st: &str, tt: &str| RelationDefinition {
            key: key.to_string(),
            status: RelationStatus::Active,
            signoff_by: Some("test-owner".to_string()),
            signoff_at: Some("2026-11-12".to_string()),
            signoff_record: Some("test".to_string()),
            registry_version: Some("1".to_string()),
            semantic_version: Some("1.0".to_string()),
            activated_at: Some("2026-11-12".to_string()),
            domain: "test".to_string(),
            owner_role: "test-owner".to_string(),
            source: "test.table".to_string(),
            source_type: st.to_string(),
            target_type: tt.to_string(),
            direction: "directed".to_string(),
            semantics: "test".to_string(),
            governance_level: Some("L2".to_string()),
            introduced_in: Some("1.0".to_string()),
            view_alias: false,
            reverse_view_of: None,
            synthetic: false,
            business_fact: true,
            type_key_source: None,
            ref_role_filter: None,
            cardinality: None,
        };
        RelationRegistry {
            version: "1".to_string(),
            relations: vec![
                active("design.covers_requirement", "design_revision", "requirement_revision"),
                active("task.implements_requirement", "task", "requirement_revision"),
                active("task.depends_on_task", "task", "task"),
            ],
            node_kinds: vec![],
        }
    }

    /// 全 draft 注册表（用于验证注册表闸）。
    fn draft_registry() -> RelationRegistry {
        let mut registry = active_registry();
        for relation in &mut registry.relations {
            relation.status = RelationStatus::Draft;
        }
        registry
    }

    fn trace_row() -> TraceEdgeRow {
        TraceEdgeRow {
            relation: "design.covers_requirement".to_string(),
            from_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            to_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            from_title: "设计 A".to_string(),
            to_title: "需求 B".to_string(),
            from_revision_id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
            to_revision_id: Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap(),
        }
    }

    fn dep_row() -> TaskDependencyRow {
        TaskDependencyRow {
            task_id: Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap(),
            depends_on_task_id: Uuid::parse_str("66666666-6666-6666-6666-666666666666").unwrap(),
            task_title: "任务 1".to_string(),
            depends_on_title: "前置任务 0".to_string(),
            task_status: "ready".to_string(),
            depends_on_status: "completed".to_string(),
        }
    }

    #[test]
    fn trace_spec_nodes_are_deduped_and_sourced() {
        let spec = build_trace_spec(Uuid::nil(), &active_registry(), &[trace_row(), trace_row()], false);
        assert_eq!(spec.nodes.len(), 2, "重复边不应产生重复节点");
        assert_eq!(spec.edges.len(), 2);
        for node in &spec.nodes {
            assert!(node.source_ref.is_some(), "Live 图节点必须有溯源");
            assert!(
                node.source_ref.as_ref().unwrap().revision_id.is_some(),
                "追踪图节点应带绑定修订"
            );
        }
    }

    #[test]
    fn task_dag_keeps_library_direction() {
        let spec = build_task_dag_spec(Uuid::nil(), &active_registry(), &[dep_row()], false);
        assert_eq!(spec.edges.len(), 1);
        // 库内语义：task_id（依赖方）→ depends_on_task_id（前置方）
        assert_eq!(spec.edges[0].from, dep_row().task_id.to_string());
        assert_eq!(spec.edges[0].to, dep_row().depends_on_task_id.to_string());
    }

    #[test]
    fn task_nodes_do_not_fake_content_revision() {
        // task 无内容版本 → 不得伪造 revision_id（0019 纪律的同类）
        let spec = build_task_dag_spec(Uuid::nil(), &active_registry(), &[dep_row()], false);
        for node in &spec.nodes {
            assert!(
                node.source_ref.as_ref().unwrap().revision_id.is_none(),
                "task 节点不得伪造内容版本"
            );
        }
    }

    #[test]
    fn truncated_flag_lowers_budget() {
        let full = build_task_dag_spec(Uuid::nil(), &active_registry(), &[dep_row()], false);
        let cut = build_task_dag_spec(Uuid::nil(), &active_registry(), &[dep_row()], true);
        assert_eq!(full.meta.budget, Budget::Faithful);
        assert_eq!(cut.meta.budget, Budget::Balanced);
    }

    #[test]
    fn draft_relations_are_not_projected() {
        // 专家裁决的落地：未 active 的关系不得出边（未会签 ≠ 默认允许）
        let trace = build_trace_spec(
            Uuid::nil(),
            &draft_registry(),
            &[trace_row()],
            false,
        );
        assert_eq!(trace.edges.len(), 0, "draft 关系不得投影出边");
        assert_eq!(trace.nodes.len(), 0, "无边即无节点");

        let dag = build_task_dag_spec(
            Uuid::nil(),
            &draft_registry(),
            &[dep_row()],
            false,
        );
        assert_eq!(dag.edges.len(), 0, "draft 的 task.depends_on_task 不得投影");
    }
}
