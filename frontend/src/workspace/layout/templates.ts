import type { LayoutTemplate, WorkspacePanel, WorkspaceSpec } from "../spec/types";

/** 每个布局模板的槽位数（spec §5.6；layout 枚举与本表同源）。 */
export const LAYOUT_SLOT_COUNT: Record<LayoutTemplate, number> = {
  overview: 2,
  split: 2,
  timeline: 1,
  explorer: 2
};

export interface ResolvedSlots {
  slots: WorkspacePanel[][];
  /** 超出槽位数的 panel 进入折叠区，不丢弃、不报错（§6.5 整理说明 4）。 */
  overflow: WorkspacePanel[];
}

/** 按数组顺序绑定槽位：前 N 个入槽，其余进溢出折叠区。 */
export function resolveSlots(spec: WorkspaceSpec): ResolvedSlots {
  const count = LAYOUT_SLOT_COUNT[spec.layout];
  const slots: WorkspacePanel[][] = Array.from({ length: count }, () => []);
  const overflow: WorkspacePanel[] = [];

  spec.panels.forEach((panel, index) => {
    if (index < count) slots[index].push(panel);
    else overflow.push(panel);
  });

  return { slots, overflow };
}