/**
 * Live 图渲染器契约（P1，ADR-VIZ-002 的前端侧）。
 *
 * 渲染器可替换（G6 / 未来备选），组件只面对本契约：
 * - capabilities 声明能力位，LiveGraphView 按位裁剪 UI；
 * - 所有交互方法只改变**视图状态**，零写路径（十号任务书 §十）。
 */

/** 渲染器能力位声明。 */
export interface VizRendererCapabilities {
  /** 渲染器标识（g6 / fallback-svg …）。 */
  rendererId: string;
  /** 悬停节点高亮入边/出边、其余降透明。 */
  hoverTrace: boolean;
  /** 点击节点回调（供 focus panel）。 */
  focusPanel: boolean;
  /** 前端 BFS 路径（route journey）。 */
  routeJourney: boolean;
  /** 按 kind / relation 过滤高亮（lens）。 */
  lens: boolean;
}

/** lens 选择：按节点 kind 与边 relation 过滤（undefined = 不过滤该维度）。 */
export interface VizLensSelection {
  kind?: string;
  relation?: string;
}

/** 渲染器实例（G6 等实现此接口）。 */
export interface VizGraphRenderer {
  readonly capabilities: VizRendererCapabilities;
  /** 释放图实例与事件监听（卸载必须调用）。 */
  destroy(): void;
  /** 应用 lens：匹配元素置 active，其余 inactive。 */
  applyLens(selection: VizLensSelection): void;
  /** 清除 lens，恢复全量展示。 */
  clearLens(): void;
  /** 高亮路径（节点序列 + 相邻边），其余降透明。 */
  highlightPath(nodeIds: string[]): void;
  /** 清除路径高亮。 */
  clearPath(): void;
  /** 在已投影边上做 BFS：起点沿边方向到终点的节点序列；不可达返回 null。 */
  findPath(fromId: string, toId: string): string[] | null;
}
