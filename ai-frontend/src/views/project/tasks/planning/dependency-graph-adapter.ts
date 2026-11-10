import type { DependencyGraph } from "@/api/task-plans";
import { shortId } from "../tasks-contract";

/**
 * S04-UI-05 Dependency Graph · 视图适配（真实读模型 → 页面模型）。
 *
 * 数据源 = `GET /v1/pm/projects/{project_id}/dependency-graph`：后端已完成
 * 分层（最长路径）、关键路径、并行组与环检测，前端**不再自行推导拓扑**，
 * 只把结果映射成可渲染的节点 / 边 / 组。
 *
 * 纪律：节点没有「模块 / 依赖原因 / 建议」字段（后端无来源）→ 一律不渲染，
 * 不用演示文案填充。
 */

/** 图节点视图模型。 */
export interface GraphNodeView {
  id: string;
  /** 展示用短编号（UUID 前 8 位）。 */
  code: string;
  title: string;
  status: string;
  priority: string;
  repo: string;
  /** 位于后端给出的关键路径上。 */
  critical: boolean;
  /** 属于某个可并行组。 */
  parallel: boolean;
  blocked: boolean;
  /** 分层序号（后端 layers 的下标）。 */
  layer: number;
}

/** 图边视图模型（from = 前置，to = 后继）。 */
export interface GraphEdgeView {
  from: string;
  to: string;
  critical: boolean;
}

/** 并行组视图模型。 */
export interface GraphGroupView {
  id: string;
  /** 展示序号（1 起）。 */
  index: number;
  taskIds: string[];
  codes: string[];
  repos: string[];
}

/** 依赖图视图模型。 */
export interface GraphViewModel {
  nodes: GraphNodeView[];
  edges: GraphEdgeView[];
  layers: string[][];
  criticalPath: string[];
  parallelGroups: GraphGroupView[];
  cycles: string[][];
}

/** 依赖图 → 页面模型。 */
export function toGraphView(graph: DependencyGraph): GraphViewModel {
  const critical = new Set(graph.critical_path ?? []);
  const parallel = new Set(
    (graph.parallel_groups ?? []).flatMap(group => group ?? [])
  );
  const layerOf = new Map<string, number>();
  (graph.layers ?? []).forEach((layer, index) => {
    for (const id of layer ?? []) layerOf.set(id, index);
  });

  const nodes: GraphNodeView[] = (graph.nodes ?? []).map(node => ({
    id: node.id,
    code: shortId(node.id),
    title: node.title,
    status: node.status,
    priority: node.priority,
    repo: node.repository_name ?? "",
    critical: critical.has(node.id),
    parallel: parallel.has(node.id),
    blocked: node.status === "blocked",
    layer: layerOf.get(node.id) ?? 0
  }));

  const edges: GraphEdgeView[] = (graph.edges ?? []).map(edge => ({
    from: edge.from_task_id,
    to: edge.to_task_id,
    critical: critical.has(edge.from_task_id) && critical.has(edge.to_task_id)
  }));

  const parallelGroups: GraphGroupView[] = (graph.parallel_groups ?? []).map(
    (group, index) => {
      const taskIds = group ?? [];
      return {
        id: `G-${index + 1}`,
        index: index + 1,
        taskIds,
        codes: taskIds.map(id => shortId(id)),
        repos: [
          ...new Set(
            nodes
              .filter(node => taskIds.includes(node.id))
              .map(node => node.repo)
              .filter(repo => repo.length > 0)
          )
        ]
      };
    }
  );

  return {
    nodes,
    edges,
    layers: graph.layers ?? [],
    criticalPath: graph.critical_path ?? [],
    parallelGroups,
    cycles: graph.cycles ?? []
  };
}

/** 校验问题（全部由真实读模型推导，无演示数据）。 */
export interface GraphIssue {
  key: string;
  text: string;
  tone: "danger" | "warning";
}

/**
 * 依赖校验：环依赖 / 前置阻塞 / 同层同仓库并行冲突。
 *
 * - 环依赖来自后端 `cycles`（成环即阻断执行规划）；
 * - 前置阻塞来自节点状态；
 * - 同层同仓库冲突 = 后端给出的并行层里共用同一仓库的任务对。
 */
export function validateGraph(
  model: GraphViewModel,
  messages: {
    cycle: (codes: string) => string;
    blockedUpstream: (code: string) => string;
    repoConflict: (a: string, b: string, repo: string) => string;
  }
): GraphIssue[] {
  const issues: GraphIssue[] = [];

  for (const cycle of model.cycles) {
    issues.push({
      key: `cycle-${cycle.join("-")}`,
      text: messages.cycle(cycle.map(id => shortId(id)).join(" → ")),
      tone: "danger"
    });
  }

  for (const edge of model.edges) {
    const upstream = model.nodes.find(node => node.id === edge.from);
    if (upstream?.blocked) {
      issues.push({
        key: `blocked-${edge.from}-${edge.to}`,
        text: messages.blockedUpstream(upstream.code),
        tone: "warning"
      });
    }
  }

  for (const layer of model.layers) {
    const members = model.nodes.filter(node => layer.includes(node.id));
    for (let i = 0; i < members.length; i += 1) {
      for (let j = i + 1; j < members.length; j += 1) {
        const a = members[i];
        const b = members[j];
        if (!a.repo || a.repo !== b.repo) continue;
        issues.push({
          key: `conflict-${a.id}-${b.id}`,
          text: messages.repoConflict(a.code, b.code, a.repo),
          tone: "warning"
        });
      }
    }
  }

  return issues;
}
