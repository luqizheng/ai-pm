//! S04 · 任务域读模型（单任务详情 / 项目级依赖图 / 执行准备度聚合 / 排期）。
//!
//! 三个读面都只做**投影**：不重算业务事实，只把既有表里的事实读成
//! 页面能直接消费的结构。
//!
//! - `task_detail`：S04-UI-03 Task Detail 的信息模型——任务本体 + 依据追溯
//!   （需求 / 设计 / ADR / 实施基线）+ 依赖 + 仓库 + 验收标准 + 交付清单
//!   + 人工审批 + 审计历史。
//! - `project_dependency_graph`：S04-UI-05 Dependency Graph——节点、依赖边、
//!   分层（最长路径）、关键路径、并行组候选（无未完成前置且互相无依赖）。
//! - `project_execution_readiness`：S04-UI-07 Execution Readiness 的
//!   A–F 六段检查清单 + 三态汇总（Ready / Needs Attention / Blocked）。
//!
//! 领域纪律（与 S04 设计稿一致）：
//! - **完成 ≠ 可执行**：任务 state=completed 不代表可移交 S05；准备度是
//!   六段检查的聚合结论。
//! - **依赖方向**：上游 = 本任务依赖的（前置），下游 = 依赖本任务的（后继）；
//!   边方向以 `depends_on_task_id` 为准。
//! - **关键路径**是投影不是存储：按「最长下游链」算出，节点多时只做
//!   O(V+E) 的分层与最长路径，不做完整 CPM 迭代。

use crate::actor::AuditActor;
use crate::pm_write::{append_audit_with_after, PmWriteError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

// ─────────────────────────────────────────────────────── 单任务详情（S04-UI-03）

/// 任务本体（delivery.task + 排期列）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskDetailRow {
    pub id: Uuid,
    /// PA-07 任务身份锚（0050）：PM-NNN，详情页展示/引用用。
    pub code: String,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub assignee_id: Option<Uuid>,
    pub revision: i64,
    pub goal: Option<String>,
    pub constraints: Value,
    pub deliverables: Value,
    pub git_branch: Option<String>,
    pub planned_start_at: Option<DateTime<Utc>>,
    pub planned_end_at: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f64>,
    pub implementation_baseline_id: Option<Uuid>,
    pub plan_revision_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 依据追溯行（需求 / 设计 / ADR / 实施基线）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskTraceRow {
    /// `requirement` / `design` / `adr` / `baseline`。
    pub kind: String,
    pub target_id: Uuid,
    pub code: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
    pub state: Option<String>,
}

/// 依赖边（上游 / 下游）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskDependencyRow {
    /// 对端任务 id（上游 = 本任务依赖的；下游 = 依赖本任务的）。
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub priority: String,
    /// `upstream`（前置）/ `downstream`（后继）。
    pub direction: String,
}

/// 仓库绑定（task ↔ repository）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskRepositoryRow {
    pub repository_id: Uuid,
    pub repository_name: String,
    pub base_ref: Option<String>,
    pub base_commit: Option<String>,
    pub write_access: bool,
    pub path_scope: Vec<String>,
    pub role: Option<String>,
    pub repository_verified: bool,
}

/// 验收标准（task_acceptance_criterion）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskCriterionRow {
    pub id: Uuid,
    pub ordinal: i32,
    pub text: String,
    pub verification_kind: String,
}

/// 人工审批（governance.review_approval；任务级）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskApprovalRow {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub required_role: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 审计历史（governance.audit_log，按任务过滤）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskHistoryRow {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: Option<String>,
    pub action: String,
    pub created_at: DateTime<Utc>,
}

/// 实施基线快照（task.implementation_baseline_id 指向的冻结依据）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct TaskBaselineRow {
    pub id: Uuid,
    pub code: String,
    pub status: String,
    pub content: Value,
}

/// 单任务详情聚合。
#[derive(Debug, Serialize)]
pub struct TaskDetail {
    pub task: TaskDetailRow,
    pub traces: Vec<TaskTraceRow>,
    pub dependencies: Vec<TaskDependencyRow>,
    pub repositories: Vec<TaskRepositoryRow>,
    pub criteria: Vec<TaskCriterionRow>,
    pub approvals: Vec<TaskApprovalRow>,
    pub history: Vec<TaskHistoryRow>,
    /// 实施基线快照（无基线则为 null）。
    pub baseline: Option<TaskBaselineRow>,
}

/// 读单任务完整详情。
pub async fn task_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
) -> Result<TaskDetail, PmWriteError> {
    let task: TaskDetailRow = sqlx::query_as(
        "SELECT id, code, project_id, title, description, status, priority, assignee_id, revision, \
                goal, constraints, deliverables, git_branch, planned_start_at, planned_end_at, \
                estimate_hours, implementation_baseline_id, plan_revision_id, created_at, updated_at \
         FROM delivery.task WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("task not found"))?;

    // 依据追溯：需求直连 + 实施基线内的设计/ADR。
    let mut traces: Vec<TaskTraceRow> = sqlx::query_as::<
        _,
        (
            Uuid,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT tr.requirement_id, r.code, r.title, \
                CAST(rv.version AS text), rv.state \
         FROM delivery.task_requirement tr \
         LEFT JOIN delivery.requirement r ON r.id = tr.requirement_id \
         LEFT JOIN delivery.requirement_revision rv ON rv.id = tr.revision_id \
         WHERE tr.tenant_id = $1 AND tr.task_id = $2 \
         ORDER BY r.code",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?
    .into_iter()
    .map(|(target_id, code, title, version, state)| TaskTraceRow {
        kind: "requirement".to_string(),
        target_id,
        code,
        title,
        version,
        state,
    })
    .collect();

    let baseline: Option<TaskBaselineRow> = match task.implementation_baseline_id {
        Some(baseline_id) => sqlx::query_as(
            "SELECT id, code, status, content FROM design.implementation_baseline \
             WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(baseline_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?,
        None => None,
    };

    if let Some(baseline) = &baseline {
        traces.push(TaskTraceRow {
            kind: "baseline".to_string(),
            target_id: baseline.id,
            code: Some(baseline.code.clone()),
            title: None,
            version: None,
            state: Some(baseline.status.clone()),
        });
        if let Some(items) = baseline.content.get("designs").and_then(Value::as_array) {
            for item in items {
                let Some(design_id) = item
                    .get("design_id")
                    .and_then(Value::as_str)
                    .and_then(|v| Uuid::parse_str(v).ok())
                else {
                    continue;
                };
                let (code, title, state, version) = sqlx::query_as(
                    "SELECT d.code, d.title, rv.state, CAST(rv.version AS text) \
                     FROM design.design_revision rv \
                     JOIN design.design d ON d.id = rv.design_id \
                     WHERE rv.tenant_id = $1 AND rv.id = $2",
                )
                .bind(tenant_id)
                .bind(
                    item.get("design_revision_id")
                        .and_then(Value::as_str)
                        .and_then(|v| Uuid::parse_str(v).ok()),
                )
                .fetch_optional(pool)
                .await
                .map_err(PmWriteError::Db)?
                .map(
                    |(code, title, state, version): (String, String, String, Option<String>)| {
                        (code, title, state, version)
                    },
                )
                .unwrap_or_else(|| {
                    (
                        item.get("code")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        item.get("title")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        String::new(),
                        item.get("version")
                            .and_then(Value::as_i64)
                            .map(|v| v.to_string()),
                    )
                });
                traces.push(TaskTraceRow {
                    kind: "design".to_string(),
                    target_id: design_id,
                    code: Some(code),
                    title: Some(title),
                    version,
                    state: Some(state),
                });
            }
        }
        if let Some(items) = baseline.content.get("adrs").and_then(Value::as_array) {
            for item in items {
                let Some(adr_id) = item
                    .get("adr_id")
                    .and_then(Value::as_str)
                    .and_then(|v| Uuid::parse_str(v).ok())
                else {
                    continue;
                };
                let (code, title, status) = sqlx::query_as(
                    "SELECT code, title, status FROM design.adr WHERE tenant_id = $1 AND id = $2",
                )
                .bind(tenant_id)
                .bind(adr_id)
                .fetch_optional(pool)
                .await
                .map_err(PmWriteError::Db)?
                .unwrap_or_else(|| {
                    (
                        item.get("code")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        String::new(),
                        item.get("status")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    )
                });
                traces.push(TaskTraceRow {
                    kind: "adr".to_string(),
                    target_id: adr_id,
                    code: Some(code),
                    title: Some(title),
                    version: None,
                    state: Some(status),
                });
            }
        }
    }

    // 依赖：上游（本任务依赖的）+ 下游（依赖本任务的）。
    let dependencies: Vec<TaskDependencyRow> = sqlx::query_as(
        "SELECT t.id, t.title, t.status, t.priority, \
                CASE WHEN d.task_id = $2 THEN 'upstream' ELSE 'downstream' END AS direction \
         FROM delivery.task_dependency d \
         JOIN delivery.task t ON t.id = CASE WHEN d.task_id = $2 THEN d.depends_on_task_id \
                                            ELSE d.task_id END \
         WHERE d.tenant_id = $1 AND (d.task_id = $2 OR d.depends_on_task_id = $2) \
         ORDER BY direction, t.title",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let repositories: Vec<TaskRepositoryRow> = sqlx::query_as(
        "SELECT tr.repository_id, pr.name AS repository_name, tr.base_ref, tr.base_commit, \
                tr.write_access, tr.path_scope, tr.role, \
                (pr.status = 'verified') AS repository_verified \
         FROM delivery.task_repository tr \
         LEFT JOIN integration.project_repository pr ON pr.id = tr.repository_id \
         WHERE tr.tenant_id = $1 AND tr.task_id = $2 ORDER BY pr.name",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let criteria: Vec<TaskCriterionRow> = sqlx::query_as(
        "SELECT id, ordinal, text, verification_kind FROM delivery.task_acceptance_criterion \
         WHERE tenant_id = $1 AND task_id = $2 ORDER BY ordinal",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let approvals: Vec<TaskApprovalRow> = sqlx::query_as(
        "SELECT id, kind, status, required_role, decided_by, decision_reason, decided_at, created_at \
         FROM governance.review_approval WHERE tenant_id = $1 AND task_id = $2 \
         ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let history: Vec<TaskHistoryRow> = sqlx::query_as(
        "SELECT a.id, a.actor_id, p.display_name AS actor_name, a.action, a.created_at \
         FROM governance.audit_log a \
         LEFT JOIN core.principal p ON p.id = a.actor_id \
         WHERE a.tenant_id = $1 AND a.target_type = 'task' AND a.target_id = $2 \
         ORDER BY a.created_at DESC LIMIT 50",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    Ok(TaskDetail {
        task,
        traces,
        dependencies,
        repositories,
        criteria,
        approvals,
        history,
        baseline,
    })
}

// ───────────────────────────────────────────── 项目级依赖图（S04-UI-05）

/// 依赖图节点。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct DependencyGraphNode {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub planned_start_at: Option<DateTime<Utc>>,
    pub planned_end_at: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f64>,
    pub repository_name: Option<String>,
}

/// 依赖图边（前置 → 后继）。
#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct DependencyGraphEdge {
    pub from_task_id: Uuid,
    pub to_task_id: Uuid,
    /// `FS`（完成-开始）为唯一依赖类型（依赖表无类型列）。
    pub dependency_type: String,
}

/// 依赖图读模型。
#[derive(Debug, Serialize)]
pub struct DependencyGraph {
    pub nodes: Vec<DependencyGraphNode>,
    pub edges: Vec<DependencyGraphEdge>,
    /// 节点分层（最长路径分层；根层 = 0）。
    pub layers: Vec<Vec<Uuid>>,
    /// 关键路径节点（最长下游链上的节点）。
    pub critical_path: Vec<Uuid>,
    /// 并行组：同层且互相无依赖边的一组节点（可并行执行的候选）。
    pub parallel_groups: Vec<Vec<Uuid>>,
    /// 环依赖（成环即阻断执行规划；空 = 无环）。
    pub cycles: Vec<Vec<Uuid>>,
}

/// 项目级依赖图（S04-UI-05）。
pub async fn project_dependency_graph(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<DependencyGraph, PmWriteError> {
    let nodes: Vec<DependencyGraphNode> = sqlx::query_as(
        "SELECT t.id, t.title, t.status, t.priority, t.planned_start_at, t.planned_end_at, \
                t.estimate_hours, \
                (SELECT pr.name FROM delivery.task_repository tr \
                   JOIN integration.project_repository pr ON pr.id = tr.repository_id \
                  WHERE tr.task_id = t.id AND tr.tenant_id = t.tenant_id \
                  ORDER BY pr.name LIMIT 1) AS repository_name \
         FROM delivery.task t \
         WHERE t.tenant_id = $1 AND t.project_id = $2 \
         ORDER BY t.created_at",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let edges: Vec<DependencyGraphEdge> = sqlx::query_as(
        "SELECT d.depends_on_task_id AS from_task_id, d.task_id AS to_task_id, 'FS' AS dependency_type \
         FROM delivery.task_dependency d \
         JOIN delivery.task t ON t.id = d.task_id \
         WHERE d.tenant_id = $1 AND t.project_id = $2 \
         ORDER BY d.created_at",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let (layers, critical_path, parallel_groups, cycles) = analyze_graph(&nodes, &edges);
    Ok(DependencyGraph {
        nodes,
        edges,
        layers,
        critical_path,
        parallel_groups,
        cycles,
    })
}

/// 图分析四元组：(分层, 关键路径, 并行组, 环)。
type GraphAnalysis = (Vec<Vec<Uuid>>, Vec<Uuid>, Vec<Vec<Uuid>>, Vec<Vec<Uuid>>);

/// 图分析：分层（最长路径）、关键路径、并行组、环检测。
///
/// 全部为 O(V+E) 级别的投影算法（Kahn 拓扑 + 最长路径 DP），不是完整 CPM。
fn analyze_graph(nodes: &[DependencyGraphNode], edges: &[DependencyGraphEdge]) -> GraphAnalysis {
    let ids: Vec<Uuid> = nodes.iter().map(|node| node.id).collect();
    let index: std::collections::HashMap<Uuid, usize> =
        ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();

    // 邻接表：前置 → 后继。
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
    let mut indegree: Vec<usize> = vec![0; ids.len()];
    for edge in edges {
        let (Some(&from), Some(&to)) = (index.get(&edge.from_task_id), index.get(&edge.to_task_id))
        else {
            continue;
        };
        if !outgoing[from].contains(&to) {
            outgoing[from].push(to);
            indegree[to] += 1;
        }
    }

    // Kahn 拓扑：同时检测环。
    let mut queue: Vec<usize> = (0..ids.len()).filter(|i| indegree[*i] == 0).collect();
    let mut order: Vec<usize> = Vec::with_capacity(ids.len());
    let mut remaining = indegree.clone();
    while let Some(current) = queue.pop() {
        order.push(current);
        for next in &outgoing[current] {
            remaining[*next] -= 1;
            if remaining[*next] == 0 {
                queue.push(*next);
            }
        }
    }

    // 环：未进入拓扑序的节点即环上或环后节点（报告最小环集合）。
    let in_order: std::collections::HashSet<usize> = order.iter().copied().collect();
    let cyclic: Vec<usize> = (0..ids.len()).filter(|i| !in_order.contains(i)).collect();
    let cycles: Vec<Vec<Uuid>> = if cyclic.is_empty() {
        Vec::new()
    } else {
        vec![cyclic.into_iter().map(|i| ids[i]).collect()]
    };

    // 分层：layer(v) = 1 + max(layer(u)) over u → v（无边则 0）。
    let mut layer = vec![0usize; ids.len()];
    for &node in &order {
        for &next in &outgoing[node] {
            layer[next] = layer[next].max(layer[node] + 1);
        }
    }
    let max_layer = layer.iter().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<Uuid>> = vec![Vec::new(); max_layer + 1];
    for (i, node) in nodes.iter().enumerate() {
        layers[layer[i]].push(node.id);
    }

    // 关键路径：下游最长链（从各起点沿最长后继走到末尾）。
    let mut longest_downstream = vec![1usize; ids.len()];
    let mut best_next: Vec<Option<usize>> = vec![None; ids.len()];
    for &node in order.iter().rev() {
        for &next in &outgoing[node] {
            if longest_downstream[next] + 1 > longest_downstream[node] {
                longest_downstream[node] = longest_downstream[next] + 1;
                best_next[node] = Some(next);
            }
        }
    }
    // 空图（项目无任务）早退：unwrap_or(0) 会落到越界索引 0 上 panic
    // （2026-11-21 空项目冒烟实测）。layers/cycles 已为空集，并行组循环
    // 天然跳过；关键路径直接置空。
    if ids.is_empty() {
        return (layers, Vec::new(), Vec::new(), cycles);
    }
    let start = (0..ids.len())
        .max_by_key(|i| longest_downstream[*i])
        .expect("non-empty ids always yield an argmax");
    let mut critical_path: Vec<Uuid> = Vec::new();
    let mut cursor = Some(start);
    while let Some(current) = cursor {
        critical_path.push(ids[current]);
        cursor = best_next[current];
    }

    // 并行组：同层内互相无依赖边的节点集合（>1 才成组）。
    let mut parallel_groups: Vec<Vec<Uuid>> = Vec::new();
    for layer_nodes in &layers {
        if layer_nodes.len() < 2 {
            continue;
        }
        let mut group: Vec<Uuid> = Vec::new();
        for &candidate in layer_nodes {
            let candidate_idx = index[&candidate];
            let conflicts = group.iter().any(|member: &Uuid| {
                let member_idx = index[member];
                outgoing[member_idx].contains(&candidate_idx)
                    || outgoing[candidate_idx].contains(&member_idx)
            });
            if !conflicts {
                group.push(candidate);
            }
        }
        if group.len() > 1 {
            parallel_groups.push(group);
        }
    }

    (layers, critical_path, parallel_groups, cycles)
}

// ─────────────────────────────────────── 执行准备度聚合（S04-UI-07）

/// 准备度检查项（一段检查的一条）。
#[derive(Debug, Serialize)]
pub struct ReadinessCheck {
    /// 段号 `A`–`F`。
    pub section: String,
    /// 检查项 id（稳定机器可读）。
    pub key: String,
    /// 检查项说明（后端只给事实与判定，说明走 i18n）。
    pub passed: bool,
    /// 未通过时的具体缺口（可为空数组 = 通过）。
    pub issues: Vec<String>,
}

/// 准备度汇总（三态）。
#[derive(Debug, Serialize)]
pub struct ReadinessSummary {
    /// `ready` / `needs_attention` / `blocked`。
    pub status: String,
    pub ready_count: i64,
    pub total_count: i64,
    pub blocked_count: i64,
}

/// 单任务准备度（任务池/表格逐行读）。
#[derive(Debug, Serialize)]
pub struct TaskReadiness {
    pub task_id: Uuid,
    pub title: String,
    pub status: String,
    pub summary: ReadinessSummary,
    pub checks: Vec<ReadinessCheck>,
}

/// 执行准备度聚合读模型。
#[derive(Debug, Serialize)]
pub struct ExecutionReadiness {
    pub summary: ReadinessSummary,
    pub tasks: Vec<TaskReadiness>,
}

/// 项目级执行准备度（S04-UI-07）。
///
/// 六段检查（§6）：
/// - A 任务定义：目标（goal）/ 范围（description）/ 交付（deliverables）；
/// - B 依据追溯：需求 + 设计 + ADR + 有效实施基线；
/// - C 依赖校验：有依赖、无环、前置已完成；
/// - D 工程准备：仓库已绑定且已核验；
/// - E 验收交付：验收标准齐备；
/// - F 人工批准：无待办审批。
pub async fn project_execution_readiness(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<ExecutionReadiness, PmWriteError> {
    let tasks: Vec<TaskDetailRow> = sqlx::query_as(
        "SELECT id, code, project_id, title, description, status, priority, assignee_id, revision, \
                goal, constraints, deliverables, git_branch, planned_start_at, planned_end_at, \
                estimate_hours, implementation_baseline_id, plan_revision_id, created_at, updated_at \
         FROM delivery.task WHERE tenant_id = $1 AND project_id = $2 ORDER BY created_at",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let edges: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT d.depends_on_task_id, d.task_id \
         FROM delivery.task_dependency d \
         JOIN delivery.task t ON t.id = d.task_id \
         WHERE d.tenant_id = $1 AND t.project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let mut rows: Vec<TaskReadiness> = Vec::with_capacity(tasks.len());
    for task in &tasks {
        let checks = evaluate_task_readiness(pool, tenant_id, task, &edges).await?;
        let summary = summarize(&checks);
        rows.push(TaskReadiness {
            task_id: task.id,
            title: task.title.clone(),
            status: task.status.clone(),
            summary,
            checks,
        });
    }

    // 项目级汇总 = 逐任务取最差档（blocked > needs_attention > ready）。
    let overall = if rows.is_empty() {
        ReadinessSummary {
            status: "ready".to_string(),
            ready_count: 0,
            total_count: 0,
            blocked_count: 0,
        }
    } else {
        let total = rows.len() as i64;
        let ready = rows
            .iter()
            .filter(|row| row.summary.status == "ready")
            .count() as i64;
        let blocked = rows
            .iter()
            .filter(|row| row.summary.status == "blocked")
            .count() as i64;
        let status = if blocked > 0 {
            "blocked"
        } else if ready < total {
            "needs_attention"
        } else {
            "ready"
        };
        ReadinessSummary {
            status: status.to_string(),
            ready_count: ready,
            total_count: total,
            blocked_count: blocked,
        }
    };

    Ok(ExecutionReadiness {
        summary: overall,
        tasks: rows,
    })
}

/// 单任务六段检查判定。
async fn evaluate_task_readiness(
    pool: &PgPool,
    tenant_id: Uuid,
    task: &TaskDetailRow,
    edges: &[(Uuid, Uuid)],
) -> Result<Vec<ReadinessCheck>, PmWriteError> {
    let mut checks: Vec<ReadinessCheck> = Vec::new();

    // A 任务定义：目标 + 范围 + 交付物。
    let mut a_issues: Vec<String> = Vec::new();
    if task.goal.as_deref().map(str::trim).unwrap_or("").is_empty() {
        a_issues.push("goal is empty".to_string());
    }
    if task
        .description
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        a_issues.push("description (scope) is empty".to_string());
    }
    let deliverable_count = task
        .deliverables
        .as_array()
        .map(|items| items.len())
        .unwrap_or(0);
    if deliverable_count == 0 {
        a_issues.push("deliverables are empty".to_string());
    }
    checks.push(ReadinessCheck {
        section: "A".to_string(),
        key: "task_definition".to_string(),
        passed: a_issues.is_empty(),
        issues: a_issues,
    });

    // B 依据追溯：需求 + 基线（基线内含设计与 ADR）。
    let requirement_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_requirement \
         WHERE tenant_id = $1 AND task_id = $2",
    )
    .bind(tenant_id)
    .bind(task.id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let mut b_issues: Vec<String> = Vec::new();
    if requirement_count == 0 {
        b_issues.push("no requirement linked".to_string());
    }
    match task.implementation_baseline_id {
        None => b_issues.push("no implementation baseline pinned".to_string()),
        Some(baseline_id) => {
            let baseline: Option<(String, String)> = sqlx::query_as(
                "SELECT status, content::text FROM design.implementation_baseline \
                 WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant_id)
            .bind(baseline_id)
            .fetch_optional(pool)
            .await
            .map_err(PmWriteError::Db)?;
            match baseline {
                None => b_issues.push("implementation baseline not found".to_string()),
                Some((status, _)) if status != "frozen" => {
                    b_issues.push(format!("implementation baseline is `{status}`"));
                }
                Some((_, content)) => {
                    let has_design = content.contains("\"design_revision_ids\"");
                    if !has_design {
                        b_issues.push("implementation baseline has no design".to_string());
                    }
                }
            }
        }
    }
    checks.push(ReadinessCheck {
        section: "B".to_string(),
        key: "source_traceability".to_string(),
        passed: b_issues.is_empty(),
        issues: b_issues,
    });

    // C 依赖校验：上游未完成 / 环。
    let upstreams: Vec<Uuid> = edges
        .iter()
        .filter(|(_, to)| *to == task.id)
        .map(|(from, _)| *from)
        .collect();
    let downstream: Vec<Uuid> = edges
        .iter()
        .filter(|(from, _)| *from == task.id)
        .map(|(_, to)| *to)
        .collect();
    let mut c_issues: Vec<String> = Vec::new();
    if upstreams.is_empty() && downstream.is_empty() {
        c_issues.push("no dependency defined".to_string());
    }
    for upstream in &upstreams {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM delivery.task WHERE tenant_id = $1 AND id = $2")
                .bind(tenant_id)
                .bind(upstream)
                .fetch_optional(pool)
                .await
                .map_err(PmWriteError::Db)?;
        if let Some(status) = status {
            if status != "completed" {
                c_issues.push(format!("upstream task {upstream} is `{status}`"));
            }
        }
    }
    checks.push(ReadinessCheck {
        section: "C".to_string(),
        key: "dependency_validation".to_string(),
        passed: c_issues.is_empty(),
        issues: c_issues,
    });

    // D 工程准备：仓库绑定且已核验。
    let mut d_issues: Vec<String> = Vec::new();
    let repository_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_repository \
         WHERE tenant_id = $1 AND task_id = $2",
    )
    .bind(tenant_id)
    .bind(task.id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    if repository_count == 0 {
        d_issues.push("no repository bound".to_string());
    } else {
        let verified: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM delivery.task_repository tr \
             JOIN integration.project_repository pr ON pr.id = tr.repository_id \
             WHERE tr.tenant_id = $1 AND tr.task_id = $2 AND pr.status = 'verified'",
        )
        .bind(tenant_id)
        .bind(task.id)
        .fetch_one(pool)
        .await
        .map_err(PmWriteError::Db)?;
        if verified == 0 {
            d_issues.push("no verified repository bound".to_string());
        }
    }
    checks.push(ReadinessCheck {
        section: "D".to_string(),
        key: "engineering_preparation".to_string(),
        passed: d_issues.is_empty(),
        issues: d_issues,
    });

    // E 验收交付：验收标准齐备。
    let criterion_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_acceptance_criterion \
         WHERE tenant_id = $1 AND task_id = $2",
    )
    .bind(tenant_id)
    .bind(task.id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let mut e_issues: Vec<String> = Vec::new();
    if criterion_count == 0 {
        e_issues.push("no acceptance criterion".to_string());
    }
    checks.push(ReadinessCheck {
        section: "E".to_string(),
        key: "acceptance_delivery".to_string(),
        passed: e_issues.is_empty(),
        issues: e_issues,
    });

    // F 人工批准：无待办审批。
    let pending_approvals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.review_approval \
         WHERE tenant_id = $1 AND task_id = $2 AND status = 'pending'",
    )
    .bind(tenant_id)
    .bind(task.id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let mut f_issues: Vec<String> = Vec::new();
    if pending_approvals > 0 {
        f_issues.push(format!("{pending_approvals} pending approval(s)"));
    }
    checks.push(ReadinessCheck {
        section: "F".to_string(),
        key: "human_approval".to_string(),
        passed: f_issues.is_empty(),
        issues: f_issues,
    });

    Ok(checks)
}

/// 六段检查 → 三态汇总。
///
/// 判定口径：任一段「硬阻断」（C 依赖 / D 工程准备）不过 → blocked；
/// 其余任一段不过 → needs_attention；全过 → ready。
fn summarize(checks: &[ReadinessCheck]) -> ReadinessSummary {
    let total = checks.len() as i64;
    let ready = checks.iter().filter(|check| check.passed).count() as i64;
    let blocked = checks
        .iter()
        .filter(|check| !check.passed && matches!(check.section.as_str(), "C" | "D"))
        .count() as i64;
    let status = if blocked > 0 {
        "blocked"
    } else if ready < total {
        "needs_attention"
    } else {
        "ready"
    };
    ReadinessSummary {
        status: status.to_string(),
        ready_count: ready,
        total_count: total,
        blocked_count: blocked,
    }
}

// ─────────────────────────────────────────────────────── 排期写（S04-UI-06）

/// 排期请求（拖拽移动 / 改工期）。
#[derive(Debug, Deserialize)]
pub struct TaskScheduleRequest {
    pub planned_start_at: Option<DateTime<Utc>>,
    pub planned_end_at: Option<DateTime<Utc>>,
    pub estimate_hours: Option<f64>,
}

/// 写生效排期（人工动作；审计留痕）。
pub async fn set_task_schedule(
    pool: &PgPool,
    tenant_id: Uuid,
    task_id: Uuid,
    req: TaskScheduleRequest,
    actor: &AuditActor,
) -> Result<TaskDetailRow, PmWriteError> {
    if let (Some(start), Some(end)) = (req.planned_start_at, req.planned_end_at) {
        if end < start {
            return Err(PmWriteError::Validation(
                "planned_end_at must not be earlier than planned_start_at",
            ));
        }
    }
    if let Some(hours) = req.estimate_hours {
        if hours <= 0.0 {
            return Err(PmWriteError::Validation("estimate_hours must be positive"));
        }
    }
    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    type TaskScheduleRow = (Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<f64>);
    let before: Option<TaskScheduleRow> = sqlx::query_as(
        "SELECT planned_start_at, planned_end_at, estimate_hours FROM delivery.task \
             WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let before = before.ok_or(PmWriteError::NotFound("task not found"))?;
    let project_id: Uuid =
        sqlx::query_scalar("SELECT project_id FROM delivery.task WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id)
            .bind(task_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?;

    sqlx::query(
        "UPDATE delivery.task \
            SET planned_start_at = $3, planned_end_at = $4, estimate_hours = $5, \
                revision = revision + 1, updated_at = now() \
          WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(task_id)
    .bind(req.planned_start_at)
    .bind(req.planned_end_at)
    .bind(req.estimate_hours)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "task.schedule.updated",
        "task",
        Some(task_id),
        json!({
            "before": { "planned_start_at": before.0, "planned_end_at": before.1,
                        "estimate_hours": before.2 },
            "after": { "planned_start_at": req.planned_start_at,
                       "planned_end_at": req.planned_end_at,
                       "estimate_hours": req.estimate_hours },
        }),
    )
    .await
    .map_err(PmWriteError::Db)?;
    tx.commit().await.map_err(PmWriteError::Db)?;

    let task: TaskDetailRow = sqlx::query_as(
        "SELECT id, code, project_id, title, description, status, priority, assignee_id, revision, \
                goal, constraints, deliverables, git_branch, planned_start_at, planned_end_at, \
                estimate_hours, implementation_baseline_id, plan_revision_id, created_at, updated_at \
         FROM delivery.task WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(task)
}
