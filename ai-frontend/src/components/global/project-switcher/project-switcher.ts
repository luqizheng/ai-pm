/**
 * ProjectSwitcher —— Figma「06 — Project Context Switcher」（33:82961）
 * Global/ProjectSwitcher 组件族的类型与常量（2026-11-04）。
 *
 * 行为约束（Figma 板注）：
 *   - Loading / Error 绝不静默切换项目上下文（错误文案明示当前项目保留）；
 *   - 列表只展示当前身份可用的项目；无权限时 Create 禁用且不泄露隐藏项目。
 */

/** 项目行的分组（面板分区头 FAVORITES / RECENT，mono 大写） */
export type ProjectSwitcherGroupId = "favorites" | "recent";

export const PROJECT_SWITCHER_GROUPS: ProjectSwitcherGroupId[] = [
  "favorites",
  "recent"
];

export interface ProjectSwitcherProject {
  id: string;
  name: string;
  group: ProjectSwitcherGroupId;
  /** 行尾元数据（如 "2h ago" / "Yesterday"）；空串隐藏 */
  meta?: string;
}

/** 面板反馈态：ready 正常列表 / loading 骨架 / empty 无可用项目 / error 加载失败 */
export type ProjectSwitcherStatus = "ready" | "loading" | "empty" | "error";

export const PROJECT_SWITCHER_STATUSES: ProjectSwitcherStatus[] = [
  "ready",
  "loading",
  "empty",
  "error"
];

/** 分区头文案（CSS 大写渲染） */
export interface ProjectSwitcherGroupLabels {
  favorites: string;
  recent: string;
}

/**
 * 组件文案（默认 Figma 33:82961 原文 = 设计参考文本；
 * 应用层经 texts 注入 i18n 文本）。
 */
export const PROJECT_SWITCHER_DEFAULT_TEXT = {
  title: "Project Switcher",
  searchPlaceholder: "Search available projects…",
  currentBadge: "Current",
  createLabel: "Create New Project",
  createUnavailableNote:
    "Creation unavailable for this identity. No hidden projects are disclosed.",
  footerNote:
    "Only available projects are listed. Create is shown when permitted.",
  emptyTitle: "No available projects",
  emptyDescription: "No projects are available to your current identity.",
  errorTitle: "Projects could not load",
  errorDescription:
    "Current project remains available. Retry to load the project list.",
  noResultsTitle: "No matching projects",
  retryLabel: "Retry",
  clearLabel: "Clear query",
  groupLabels: { favorites: "Favorites", recent: "Recent" }
};

export type ProjectSwitcherText = typeof PROJECT_SWITCHER_DEFAULT_TEXT;
