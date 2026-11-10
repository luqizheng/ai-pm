//! 受控跨域关系遍历（WP-2）。
//!
//! **架构定论**（WP-2 开篇金句）：正式关系遍历负责**解释影响路径**，文本相似度
//! 负责**发现候选证据**——两者互补，不混为一谈。本模块只产正式关系路径。
//!
//! **五项实施约束**（WP-2 裁决）：
//! 1. **方向与关系语义分离**——反向遍历只改路径方向，不改关系语义：输出
//!    `traversal_direction=Reverse`，**绝不虚构关系类型**（不得把「Requirement
//!    ←reverse: constrains— Rule」输出成 enforced_by）。
//! 2. **多路径保留**——同一终点经 Rule/Capability/Object 的多条独立路径分别保留；
//!    直达与间接**不合并**成无路径差异的结论。
//! 3. **超时语义**——超时返回已遍历部分 + `truncated=true` + `truncation_reason`；
//!    调用方不得据此宣称完整影响分析。预算由**统一请求预算**控制，不每跳重置。
//! 4. **gaps 六分类**——不把缺失一锅烩成「关系不存在」。
//! 5. **双端隔离**——每跳租户/项目硬过滤之外，**关系两端端点对象**也必须同租户
//!    同项目（经 `pc.endpoint_in_scope` 判定，防跨项目引用脏数据经端点连接渗入）。
//!
//! **边集**：V1 只遍历 `pc.relationship`（`lifecycle_status='active'`）——
//! draft / speculative / 引用四元组一律不入遍历（引用边接口预留、后续单独评估）。
//!
//! **防环**：`path` 数组（已访问节点）+ 跳数上限**双保险**——关系图成环时不无限
//! 遍历、不重复展开。

use std::collections::BTreeSet;

use sqlx::{PgPool, Row};
use uuid::Uuid;

use kb_pc_contracts::retrieval::{
    TraversalDirection, TraversalEdge, TraversalGap, TraversalResult,
};

/// 遍历请求。
#[derive(Debug, Clone)]
pub struct TraversalRequest {
    /// 种子节点（规范标识：`pc:{uuid}` / `knowledge_base:{uuid}` /
    /// `requirement:{uuid}` / `delivery:{uuid}`）。
    pub seed_nodes: Vec<String>,
    /// 最大跳数（1~5；默认 3）。
    pub max_hops: u8,
    /// 最大候选边数（1~60）。
    pub max_candidates: usize,
    /// 时间预算（毫秒；统一请求预算，不每跳重置）。
    pub timeout_ms: u64,
}

/// 遍历 SQL（**预定义常量**——LLM 永不生成 SQL）。
///
/// 结构：① `edges` 把关系行规范化为 (src, tgt) 字符串节点并施加**双端隔离**与
/// **类型白名单**；② `walk` 递归展开，双向跟随（正向/反向），`path` 数组防环。
const TRAVERSAL_SQL: &str = r#"
WITH RECURSIVE edges AS (
    SELECT r.id::text AS relationship_id,
           r.relationship_type_key,
           CASE WHEN r.source_object_id IS NOT NULL THEN 'pc:' || r.source_object_id::text
                ELSE r.source_ext_domain || ':' || r.source_ext_id END AS src,
           CASE WHEN r.target_object_id IS NOT NULL THEN 'pc:' || r.target_object_id::text
                ELSE r.target_ext_domain || ':' || r.target_ext_id END AS tgt
      FROM pc.relationship r
     WHERE r.lifecycle_status = 'active'
       AND r.tenant_id = $1
       AND r.project_id = $2
       AND EXISTS (SELECT 1 FROM pc.relationship_type t
                    WHERE t.type_key = r.relationship_type_key AND t.status = 'active')
       AND pc.endpoint_in_scope(r.source_object_id, r.source_ext_domain, r.source_ext_type,
                                r.source_ext_id, $1, $2)
       AND pc.endpoint_in_scope(r.target_object_id, r.target_ext_domain, r.target_ext_type,
                                r.target_ext_id, $1, $2)
),
walk AS (
    SELECT e.relationship_id,
           e.relationship_type_key,
           CASE WHEN e.src = ANY($3) THEN e.src ELSE e.tgt END AS from_node,
           CASE WHEN e.src = ANY($3) THEN e.tgt ELSE e.src END AS to_node,
           1 AS depth,
           ARRAY[CASE WHEN e.src = ANY($3) THEN e.src ELSE e.tgt END,
                 CASE WHEN e.src = ANY($3) THEN e.tgt ELSE e.src END]::text[] AS path,
           CASE WHEN e.src = ANY($3) THEN 'forward' ELSE 'reverse' END AS direction
      FROM edges e
     WHERE e.src = ANY($3) OR e.tgt = ANY($3)
    UNION ALL
    SELECT e.relationship_id,
           e.relationship_type_key,
           w.to_node AS from_node,
           CASE WHEN e.src = w.to_node THEN e.tgt ELSE e.src END AS to_node,
           w.depth + 1,
           w.path || (CASE WHEN e.src = w.to_node THEN e.tgt ELSE e.src END),
           CASE WHEN e.src = w.to_node THEN 'forward' ELSE 'reverse' END
      FROM walk w
      JOIN edges e ON e.src = w.to_node OR e.tgt = w.to_node
     WHERE w.depth < $4
       AND NOT (CASE WHEN e.src = w.to_node THEN e.tgt ELSE e.src END) = ANY(w.path)
)
SELECT relationship_id, relationship_type_key, from_node, to_node, depth, direction, path
  FROM walk
 ORDER BY depth, relationship_id, from_node, to_node
 LIMIT $5
"#;

/// 受控遍历。
pub async fn traverse(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    request: &TraversalRequest,
) -> TraversalResult {
    if request.seed_nodes.is_empty() {
        return TraversalResult::default();
    }
    let hops = request.max_hops.clamp(1, 5);
    let limit = request.max_candidates.clamp(1, 60);

    let query = sqlx::query(TRAVERSAL_SQL)
        .bind(tenant_id)
        .bind(project_id)
        .bind(&request.seed_nodes)
        .bind(hops as i32)
        .bind(limit as i64);

    let rows = match tokio::time::timeout(
        std::time::Duration::from_millis(request.timeout_ms),
        query.fetch_all(pool),
    )
    .await
    {
        Ok(Ok(rows)) => rows,
        Ok(Err(err)) => {
            tracing::error!(error = %err, "retrieval traversal query failed");
            return TraversalResult {
                gaps: vec![TraversalGap {
                    kind: "traversal_timeout".into(),
                    detail: format!("traversal query failed: {err}"),
                }],
                truncated: true,
                truncation_reason: Some("traversal_error".into()),
                ..Default::default()
            };
        }
        Err(_) => {
            // 超时：返回**已遍历部分**——本实现整体回滚（事务内查询），故如实报
            // 缺口，不伪造部分结果。
            return TraversalResult {
                gaps: vec![TraversalGap {
                    kind: "traversal_timeout".into(),
                    detail: format!("traversal exceeded {}ms budget", request.timeout_ms),
                }],
                truncated: true,
                truncation_reason: Some("traversal_timeout".into()),
                ..Default::default()
            };
        }
    };

    let mut edges: Vec<TraversalEdge> = Vec::new();
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    let mut paths: Vec<Vec<String>> = Vec::new();
    let mut gaps: Vec<TraversalGap> = Vec::new();
    let mut max_depth: i32 = 0;

    for row in rows {
        let relationship_id: String = row.get("relationship_id");
        let relationship_type: String = row.get("relationship_type_key");
        let from_node: String = row.get("from_node");
        let to_node: String = row.get("to_node");
        let direction: String = row.get("direction");
        let path: Vec<String> = row.get("path");
        let depth: i32 = row.get("depth");
        max_depth = max_depth.max(depth);

        nodes.insert(from_node.clone());
        nodes.insert(to_node.clone());
        if !paths.contains(&path) {
            paths.push(path);
        }
        edges.push(TraversalEdge {
            relationship_id,
            relationship_type,
            source_id: from_node,
            target_id: to_node,
            traversal_direction: match direction.as_str() {
                "reverse" => TraversalDirection::Reverse,
                _ => TraversalDirection::Forward,
            },
        });
    }

    // 种子命中但零正式边：如实报告缺口（RT-011——"关系不存在"要可见，不静默）。
    if edges.is_empty() && !request.seed_nodes.is_empty() {
        gaps.push(TraversalGap {
            kind: "no_active_edge".into(),
            detail: format!(
                "seeds resolved ({}) but no active formal relationship edges found",
                request.seed_nodes.len()
            ),
        });
    }
    // 候选预算耗尽：取满上限即可能还有更多。
    if edges.len() >= limit {
        gaps.push(TraversalGap {
            kind: "candidate_budget_exceeded".into(),
            detail: format!("traversal returned {limit} edges (budget cap); more may exist"),
        });
    }
    // 深度上限：走到跳数上限即可能还有更深路径（明确告知，不静默截断）。
    if max_depth >= hops as i32 {
        gaps.push(TraversalGap {
            kind: "traversal_depth_exceeded".into(),
            detail: format!("traversal reached depth limit {hops}; deeper paths may exist"),
        });
    }

    TraversalResult {
        nodes: nodes.into_iter().collect(),
        edges,
        gaps,
        truncated: false,
        truncation_reason: None,
    }
}

/// 解析实体锚点为规范节点标识（PC 对象 / KB 文档）。
///
/// 归属校验在**本函数**完成——锚点指向的对象必须属于该项目，否则不产出节点
/// （不跨项目取数）。
pub async fn resolve_seed_nodes(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    anchors: &[kb_pc_contracts::retrieval::EntityAnchor],
    limit: usize,
) -> Vec<String> {
    let mut nodes: Vec<String> = Vec::new();
    for anchor in anchors {
        if nodes.len() >= limit {
            break;
        }
        match anchor.kind.as_str() {
            "pc_object" => {
                let ids: Vec<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM pc.cognition_object \
                      WHERE tenant_id = $1 AND project_id = $2 \
                        AND (object_key = $3 OR display_name = $3) \
                      ORDER BY updated_at DESC LIMIT 4",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(&anchor.key_or_name)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
                for id in ids {
                    nodes.push(format!("pc:{id}"));
                }
            }
            "kb_document" => {
                let ids: Vec<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM kb.document \
                      WHERE tenant_id = $1 AND project_id = $2 AND doc_key = $3 LIMIT 4",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(&anchor.key_or_name)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
                for id in ids {
                    nodes.push(format!("knowledge_base:{id}"));
                }
            }
            _ => {}
        }
    }
    nodes
}
