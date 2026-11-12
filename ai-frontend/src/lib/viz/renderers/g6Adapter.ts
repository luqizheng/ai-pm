/**
 * G6 5 按需注册（V-1 第 4 项：布局算法按需注册，只注册用到的）。
 *
 * 为什么不走 `@antv/g6` 主入口：主入口 `import './preset'` 会全量注册
 * 全部内置 behaviors/layouts/plugins/elements/themes，tree-shaking 失效
 * （实测 1410KB raw / 405KB gzip，超 350KB 上限）。本模块从
 * `@antv/g6/esm/exports`（无 preset 副作用）取符号并手动注册最小集。
 */
import {
  AntVDagreLayout,
  Circle,
  DragCanvas,
  DragElement,
  ExtensionCategory,
  Graph,
  Line,
  OptimizeViewportTransform,
  ZoomCanvas,
  register
} from "@antv/g6/esm/exports";
// 箭头端点计算与绘制顺序（默认 transforms，渲染必需）——未入公开导出面，走深路径
import { ArrangeDrawOrder, GetEdgeActualEnds } from "@antv/g6/esm/transforms";

import type {
  EdgeOptions,
  NodeOptions,
  LayoutOptions
} from "@antv/g6/esm/exports";
import type { VizSpec } from "../types";
import type {
  VizGraphRenderer,
  VizLensSelection,
  VizRendererCapabilities
} from "./capabilities";
import {
  collectKinds,
  collectRelations,
  edgeMatchesSelection,
  findPathInSpec,
  nodeMatchesSelection,
  pathEdgeIds,
  toG6Data
} from "./graphLogic";

register(ExtensionCategory.NODE, "circle", Circle);
register(ExtensionCategory.EDGE, "line", Line);
register(ExtensionCategory.BEHAVIOR, "drag-canvas", DragCanvas);
register(ExtensionCategory.BEHAVIOR, "zoom-canvas", ZoomCanvas);
register(ExtensionCategory.BEHAVIOR, "drag-element", DragElement);

register(ExtensionCategory.LAYOUT, "antv-dagre", AntVDagreLayout);

register(
  ExtensionCategory.TRANSFORM,
  "get-edge-actual-ends",
  GetEdgeActualEnds
);
register(ExtensionCategory.TRANSFORM, "arrange-draw-order", ArrangeDrawOrder);
// 视口变换（拖画布/缩放）时跳过标签重排与逐帧全量重绘——150 节点交互帧率关键优化
// （G6 5.1 中它是 BEHAVIOR 分类，不是 transform）
register(
  ExtensionCategory.BEHAVIOR,
  "optimize-viewport-transform",
  OptimizeViewportTransform
);

/**
 * G6 5 渲染器适配器（P1，ADR-VIZ-002）。
 *
 * 安全边界（十号任务书 §2 硬约束）：
 * - **禁用 HTML shape**：只用绘制形状（circle/line）+ 内部文本，
 *   不注册任何 `html` 类型节点——HTML 内嵌脚本是 XSS 面；
 * - 渲染数据只来自服务端已裁剪的 Live spec，本模块不取数、零写路径。
 *
 * 加载纪律：本模块 **只能被动态 import**（LiveGraphView 内 `await import(...)`），
 * 保证 G6 全量落在 lazy chunk，不污染主 bundle（V-1 / 门禁脚本检查）。
 *
 * 交互（capabilities 全 true）：
 * - hover trace：增量高亮实现（下方 applyHover，非内置 hover-activate）；
 * - focus：node:click 回调（focus panel 由组件层接 VizTracePanel）；
 * - route journey：`findPath`（graphLogic 纯 BFS，只走已投影边）；
 * - lens：`applyLens`（kind/relation 过滤高亮）。
 */

/** 能力位：G6 实现全量支持四项交互。 */
export const G6_CAPABILITIES: VizRendererCapabilities = {
  rendererId: "g6",
  hoverTrace: true,
  focusPanel: true,
  routeJourney: true,
  lens: true
};

/** 从 tokens.css 读颜色（视觉数值唯一真源纪律：不写裸色值）。 */
function cssVar(name: string): string {
  if (typeof window === "undefined") return "";
  return window
    .getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
}

/** 布局映射：spec.view.layout_hint → G6 布局（presentation transform，不回写 spec）。
 * 只注册 antv-dagre 一种布局（V-1 按需纪律）：无 hint 或未知 hint 一律分层布局。 */
function resolveLayout(hint?: string): LayoutOptions {
  if (hint === "layered-tb") {
    return { type: "antv-dagre", rankdir: "TB", nodesep: 16, ranksep: 48 };
  }
  // layered-lr 与无 hint 的默认
  return { type: "antv-dagre", rankdir: "LR", nodesep: 16, ranksep: 48 };
}

/** 标签预算：超过该节点数默认不渲染标签，仅在 active（hover/lens/path）时显示。
 * 150 节点全量标签是 canvas 重绘大头（实测 47.8 → 53.4fps），小图无此压力。 */
const LABEL_BUDGET = 80;

function nodeOptions(showAllLabels: boolean): NodeOptions {
  const readLabel = (d: Record<string, unknown>) =>
    String((d.data as { label?: string })?.label ?? "");
  return {
    style: {
      size: 28,
      fill: cssVar("--surface-raised"),
      stroke: cssVar("--border-strong"),
      lineWidth: 1,
      labelText: showAllLabels ? readLabel : "",
      labelFill: cssVar("--text-primary"),
      labelFontSize: 12,
      labelPlacement: "bottom"
      // 不渲染节点内 icon 文本：150 次 fillText/帧是 canvas 重绘大头，
      // 类型辨识由 lens 过滤与表格视图承载（P1 帧率优化实测 2026-11-12）
    },
    state: {
      active: {
        lineWidth: 2,
        stroke: cssVar("--accent"),
        labelFontWeight: 600,
        // 大图：激活态才补偿标签，保证 hover trace 可读
        ...(showAllLabels ? {} : { labelText: readLabel })
      },
      inactive: {
        fillOpacity: 0.25,
        strokeOpacity: 0.25,
        labelOpacity: 0.25
      }
    }
  };
}

function edgeOptions(): EdgeOptions {
  return {
    style: {
      stroke: cssVar("--border-strong"),
      lineWidth: 1,
      endArrow: true,
      endArrowSize: 6
      // 边不渲染标签：150+ 边的全量文本/背景是主要渲染开销，
      // 关系语义经 hover 高亮 / lens / 表格视图承载（P1 减负实测 2026-11-12）
    },
    state: {
      active: {
        stroke: cssVar("--accent"),
        lineWidth: 2
      },
      inactive: {
        strokeOpacity: 0.15
      }
    }
  };
}

export interface CreateG6RendererOptions {
  container: HTMLElement;
  spec: VizSpec;
  onNodeClick?: (nodeId: string) => void;
  onReady?: () => void;
}

/** 创建 G6 渲染器（失败抛错，由调用方降级表格）。 */
export async function createG6Renderer(
  options: CreateG6RendererOptions
): Promise<VizGraphRenderer> {
  const { container, spec } = options;
  const data = toG6Data(spec);

  const graph = new Graph({
    container,
    animation: false,
    autoFit: "view",
    padding: 24,
    // 使用 G6 默认 canvas renderer：G6 runtime 会强制关闭脏矩形渲染，
    // SVG renderer 每次 setState 走 DOM 全量 diff，实测更慢（32.8 vs 50.3fps）。
    data,
    node: nodeOptions(data.nodes.length <= LABEL_BUDGET),
    edge: edgeOptions(),
    layout: resolveLayout(spec.view.layout_hint),
    // 画布基础操作；hover trace 不用内置 hover-activate（每次 pointer 事件
    // 对全量元素 setState，150 节点下实测把帧率拖到 33fps）——见下方增量实现
    behaviors: [
      "optimize-viewport-transform",
      "drag-canvas",
      "zoom-canvas",
      "drag-element"
    ]
  });

  graph.on("node:click", (event: unknown) => {
    // v5 事件对象：IPointerEvent.target.id（target 为命中的图元素）
    const nodeId = (event as { target?: { id?: string } }).target?.id;
    if (nodeId && options.onNodeClick) options.onNodeClick(nodeId);
  });

  // ── hover trace（增量实现）─────────────────────────────────────────
  // 语义（十号任务书）：悬停节点高亮 1 度邻居，其余降透明。
  // 内置 hover-activate 每次 pointerenter/leave 都对全量元素 setState；
  // 这里维护「当前高亮目标」，只对差集元素 setState（270 → ~10 个/次）。
  const neighborsOf = new Map<
    string,
    { nodes: Set<string>; edges: Set<string> }
  >();
  for (const e of data.edges) {
    for (const [self, other] of [
      [e.source, e.target],
      [e.target, e.source]
    ] as const) {
      const entry = neighborsOf.get(self) ?? {
        nodes: new Set<string>(),
        edges: new Set<string>()
      };
      entry.nodes.add(other);
      entry.edges.add(e.id);
      neighborsOf.set(self, entry);
    }
  }

  /** lens/path 激活期间 hover 不生效（用户显式视图状态优先）。 */
  let viewOverride = false;
  let hoverTarget: string | null = null;

  async function applyHover(nodeId: string | null): Promise<void> {
    if (viewOverride) return;
    const states: Record<string, string[]> = {};
    if (nodeId === null) {
      if (hoverTarget === null) return;
      // 离开节点：只复位原目标的邻居（其余保持 inactive），鼠标离开画布时全量复位
      const prev = neighborsOf.get(hoverTarget);
      if (prev) {
        for (const id of prev.nodes) states[id] = [];
        for (const id of prev.edges) states[id] = [];
      }
      hoverTarget = null;
    } else {
      const next = neighborsOf.get(nodeId) ?? {
        nodes: new Set([nodeId]),
        edges: new Set<string>()
      };
      if (hoverTarget === null) {
        // 首次进入：其余全量降透明（一次性）+ 邻居高亮
        for (const n of data.nodes) {
          states[n.id] = next.nodes.has(n.id) ? ["active"] : ["inactive"];
        }
        for (const e of data.edges) {
          states[e.id] = next.edges.has(e.id) ? ["active"] : ["inactive"];
        }
      } else if (hoverTarget !== nodeId) {
        // 换目标：差集增量
        const prev = neighborsOf.get(hoverTarget);
        const prevNodes = prev?.nodes ?? new Set<string>();
        const prevEdges = prev?.edges ?? new Set<string>();
        for (const id of prevNodes) {
          if (!next.nodes.has(id)) states[id] = [];
        }
        for (const id of next.nodes) {
          if (!prevNodes.has(id)) states[id] = ["active"];
        }
        for (const id of prevEdges) {
          if (!next.edges.has(id)) states[id] = [];
        }
        for (const id of next.edges) {
          if (!prevEdges.has(id)) states[id] = ["active"];
        }
        if (!next.nodes.has(hoverTarget)) states[hoverTarget] = ["inactive"];
        states[nodeId] = ["active"];
      } else {
        return;
      }
      hoverTarget = nodeId;
    }
    if (Object.keys(states).length > 0) {
      await graph.setElementState(states, false);
    }
  }

  // 单飞合并：高频 pointerenter 只保留最后一次待执行，避免每次事件都全量重绘
  // （150 节点下单次 setElementState 重绘约 30ms，扫掠式移动会连续丢弃帧）。
  type HoverJob = { kind: "hover"; id: string | null } | { kind: "reset" };
  let hoverBusy = false;
  let hoverPending: HoverJob | null = null;

  function scheduleHover(job: HoverJob): void {
    hoverPending = job;
    if (hoverBusy) return;
    hoverBusy = true;
    void (async () => {
      try {
        while (hoverPending) {
          const current = hoverPending;
          hoverPending = null;
          if (current.kind === "hover") {
            await applyHover(current.id);
          } else {
            hoverTarget = null;
            const states: Record<string, string[]> = {};
            for (const n of data.nodes) states[n.id] = [];
            for (const e of data.edges) states[e.id] = [];
            await graph.setElementState(states, false);
          }
        }
      } finally {
        hoverBusy = false;
      }
    })();
  }

  graph.on("node:pointerenter", (event: unknown) => {
    const id = (event as { target?: { id?: string } }).target?.id;
    if (id) scheduleHover({ kind: "hover", id });
  });
  // node:pointerleave 不复位：快速移动时 leave→enter 交替会把目标置空，
  // 导致下一次 enter 回落「全量 setState」分支（实测把帧率拖回 33fps）。
  // 增量基准保留在 hoverTarget 上，鼠标彻底离开画布才全量复位。
  graph.on("canvas:pointerleave", () => {
    // lens/path 激活期间鼠标离开画布不得复位用户显式视图状态
    if (viewOverride) return;
    scheduleHover({ kind: "reset" });
  });

  await graph.render();
  options.onReady?.();

  const allNodeIds = data.nodes.map(n => n.id);
  const allEdgeIds = data.edges.map(e => e.id);

  async function resetStates(): Promise<void> {
    const stateMap: Record<string, string[]> = {};
    for (const id of allNodeIds) stateMap[id] = [];
    for (const id of allEdgeIds) stateMap[id] = [];
    await graph.setElementState(stateMap, false);
  }

  async function suppressAllExcept(
    activeNodes: string[],
    activeEdges: string[]
  ): Promise<void> {
    const activeNodeSet = new Set(activeNodes);
    const activeEdgeSet = new Set(activeEdges);
    const stateMap: Record<string, string[]> = {};
    for (const id of allNodeIds) {
      stateMap[id] = activeNodeSet.has(id) ? ["active"] : ["inactive"];
    }
    for (const id of allEdgeIds) {
      stateMap[id] = activeEdgeSet.has(id) ? ["active"] : ["inactive"];
    }

    await graph.setElementState(stateMap, false);
  }

  return {
    capabilities: G6_CAPABILITIES,
    destroy() {
      graph.destroy();
    },
    async applyLens(selection: VizLensSelection) {
      viewOverride = true;
      hoverTarget = null;
      const activeNodes = spec.nodes
        .filter(n => nodeMatchesSelection(spec, n, selection))
        .map(n => n.id);
      const activeEdges = spec.edges
        .filter(e => edgeMatchesSelection(e, selection))
        .map(e => e.id);
      await suppressAllExcept(activeNodes, activeEdges);
    },
    async clearLens() {
      viewOverride = false;
      await resetStates();
    },
    async highlightPath(nodeIds: string[]) {
      viewOverride = true;
      hoverTarget = null;
      const edgeIds = pathEdgeIds(spec, nodeIds);
      await suppressAllExcept(nodeIds, edgeIds);
    },
    async clearPath() {
      viewOverride = false;
      await resetStates();
    },
    findPath(fromId: string, toId: string) {
      return findPathInSpec(spec, fromId, toId);
    }
  };
}

/** lens 下拉数据源（透传 graphLogic，组件层不用直接依赖纯函数模块）。 */
export const g6LensOptions = (spec: VizSpec) => ({
  kinds: collectKinds(spec),
  relations: collectRelations(spec)
});
