/**
 * WorkspaceSpec 协议类型（hybrid workspace design §6.5）。
 *
 * 协议层刻意宽松：`component` 是字符串、`config` 是自由对象；类型安全由
 * 协议校验 → 组件注册表 → 数据引用三层补全（见 schema.ts / registry.ts）。
 */
export const LAYOUT_TEMPLATES = ["overview", "split", "timeline", "explorer"] as const;
export type LayoutTemplate = (typeof LAYOUT_TEMPLATES)[number];

/** /focus 的 P0 子集（附录 C.1）。 */
export const FOCUS_OBJECT_TYPES = ["requirement", "task"] as const;
export type FocusObjectType = (typeof FOCUS_OBJECT_TYPES)[number];

export interface DataRef {
  type: string;
  id: string;
}

export interface WorkspacePanel {
  /** 注册表 id（registry.ts）。未注册时降级为 FallbackPanel。 */
  component: string;
  dataRef?: DataRef;
  config?: Record<string, unknown>;
}

export interface WorkspaceSpec {
  schemaVersion: "1.0";
  workspaceId: string;
  projectId: string;
  subject?: DataRef;
  layout: LayoutTemplate;
  panels: WorkspacePanel[];
}