/**
 * Live 图纯逻辑（**零 G6 依赖**，可单测）。
 *
 * 为什么独立：G6 Graph 需要 canvas 环境，happy-dom 无法实例化；
 * 路径查找 / lens 匹配 / 数据映射等纯逻辑拆出，vitest 直接测，
 * G6 交互留给 Playwright e2e。
 */
import type { VizEdge, VizNode, VizSpec } from "../types";
import type { VizLensSelection } from "./capabilities";

/** BFS 单次规模上限（与后端 LIVE_EDGE_LIMIT 对齐的防御值）。 */
export const JOURNEY_MAX_NODES = 500;

/** G6 数据节点（GraphData 要求 index signature）。 */
export interface G6NodeDatum {
  [key: string]: unknown;
  id: string;
  data: {
    label: string;
    kind: string;
    status?: string;
  };
}

/** G6 数据边。 */
export interface G6EdgeDatum {
  [key: string]: unknown;
  id: string;
  source: string;
  target: string;
  data: {
    relation: string;
    label?: string;
  };
}

/** spec → G6 data（VizEdge.from/to → G6 source/target）。 */
export function toG6Data(spec: VizSpec): {
  nodes: G6NodeDatum[];
  edges: G6EdgeDatum[];
} {
  const nodeIds = new Set(spec.nodes.map(n => n.id));
  return {
    nodes: spec.nodes.map(n => toG6Node(n)),
    // 悬空边（端点不在节点集）不进渲染数据——G6 会因悬空引用渲染异常
    edges: spec.edges
      .filter(e => nodeIds.has(e.from) && nodeIds.has(e.to))
      .map(e => toG6Edge(e))
  };
}

function toG6Node(node: VizNode): G6NodeDatum {
  return {
    id: node.id,
    data: {
      label: node.label,
      kind: node.kind,
      ...(node.status !== undefined ? { status: node.status } : {})
    }
  };
}

function toG6Edge(edge: VizEdge): G6EdgeDatum {
  return {
    id: edge.id,
    source: edge.from,
    target: edge.to,
    data: {
      relation: edge.relation,
      ...(edge.label !== undefined ? { label: edge.label } : {})
    }
  };
}

/**
 * route journey：已投影边上的单向 BFS（起点沿边方向 → 终点）。
 *
 * 纪律：只走 spec 内的边——spec 已被服务端权限裁剪，前端不得为了
 * "找到路径"引入图外数据（十号任务书 §十）。
 */
export function findPathInSpec(
  spec: Pick<VizSpec, "nodes" | "edges">,
  fromId: string,
  toId: string
): string[] | null {
  if (fromId === toId) {
    return spec.nodes.some(n => n.id === fromId) ? [fromId] : null;
  }
  if (spec.nodes.length > JOURNEY_MAX_NODES) {
    // 超防御上限不做 BFS（当前 LIVE_EDGE_LIMIT=500，正常不会触达）
    return null;
  }
  const adjacency = new Map<string, string[]>();
  for (const edge of spec.edges) {
    const list = adjacency.get(edge.from) ?? [];
    list.push(edge.to);
    adjacency.set(edge.from, list);
  }
  const visited = new Set<string>([fromId]);
  // 记录前驱以重建路径（防环：visited 入队即标记）
  const parent = new Map<string, string>();
  const queue: string[] = [fromId];
  while (queue.length > 0) {
    const current = queue.shift() as string;
    for (const next of adjacency.get(current) ?? []) {
      if (visited.has(next)) continue;
      visited.add(next);
      parent.set(next, current);
      if (next === toId) {
        const path = [next];
        let step = current;
        while (step !== fromId) {
          path.unshift(step);
          step = parent.get(step) as string;
        }
        path.unshift(fromId);
        return path;
      }
      queue.push(next);
    }
  }
  return null;
}

/** 边是否命中 lens 选择（relation 维度）。 */
export function edgeMatchesSelection(
  edge: VizEdge,
  selection: VizLensSelection
): boolean {
  if (
    selection.relation !== undefined &&
    edge.relation !== selection.relation
  ) {
    return false;
  }
  return true;
}

/** 节点是否命中 lens 选择（kind 维度 + 邻接边 relation 维度）。 */
export function nodeMatchesSelection(
  spec: Pick<VizSpec, "nodes" | "edges">,
  node: VizNode,
  selection: VizLensSelection
): boolean {
  if (selection.kind !== undefined && node.kind !== selection.kind) {
    return false;
  }
  if (selection.relation === undefined) return true;
  // relation 维度：节点至少关联一条命中边才保留高亮
  return spec.edges.some(
    e =>
      (e.from === node.id || e.to === node.id) &&
      edgeMatchesSelection(e, selection)
  );
}

/** spec 内全部去重节点 kind（lens 下拉数据源）。 */
export function collectKinds(spec: Pick<VizSpec, "nodes">): string[] {
  return Array.from(new Set(spec.nodes.map(n => n.kind))).sort();
}

/** spec 内全部去重边 relation（lens 下拉数据源）。 */
export function collectRelations(spec: Pick<VizSpec, "edges">): string[] {
  return Array.from(new Set(spec.edges.map(e => e.relation))).sort();
}

/** 路径节点序列 → 相邻对之间的边 id 序列（highlightPath 用）。 */
export function pathEdgeIds(
  spec: Pick<VizSpec, "edges">,
  path: string[]
): string[] {
  const ids: string[] = [];
  for (let i = 0; i < path.length - 1; i++) {
    const from = path[i];
    const to = path[i + 1];
    const edge = spec.edges.find(e => e.from === from && e.to === to);
    if (edge) ids.push(edge.id);
  }
  return ids;
}
