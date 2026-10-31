import type { Component } from "vue";

/**
 * 组件注册表（workspace design §5.1）：spec 里的 `component` 只是字符串，
 * 只有注册过的 id 才能渲染；未注册的一律降级为 FallbackPanel。
 */
export interface WorkspaceComponentDef {
  id: string;
  component: Component;
  /** config 校验；返回错误列表（空数组 = 合法）。 */
  validateConfig?: (config: Record<string, unknown> | undefined) => string[];
}

const registry = new Map<string, WorkspaceComponentDef>();

export function registerWorkspaceComponent(def: WorkspaceComponentDef): void {
  registry.set(def.id, def);
}

export function getWorkspaceComponent(id: string): WorkspaceComponentDef | undefined {
  return registry.get(id);
}

export function validateProps(
  def: WorkspaceComponentDef,
  config?: Record<string, unknown>
): string[] {
  return def.validateConfig ? def.validateConfig(config) : [];
}

/** 仅测试使用：清空注册表，避免用例之间串味。 */
export function resetWorkspaceRegistry(): void {
  registry.clear();
}