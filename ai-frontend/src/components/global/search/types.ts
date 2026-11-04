/**
 * Global/Search 领域类型（Figma「01 — Global Search」/「02 — Search Results」）。
 *
 * 可搜索对象固定为六类（设计任务书 §4A）；权限感知由数据层保证——
 * 搜索函数只返回当前用户可访问的对象，面板按「Available objects only」口径展示。
 */

/** 六类可搜索对象（Figma ResultRow ObjectType 变体顺序） */
export type SearchObjectType =
  | "project"
  | "repository"
  | "requirement"
  | "task"
  | "aiRun"
  | "document";

/** 分类展示顺序（Figma Populated 结果列表自上而下） */
export const SEARCH_OBJECT_TYPES: SearchObjectType[] = [
  "project",
  "repository",
  "requirement",
  "task",
  "aiRun",
  "document"
];

/** 一条搜索结果（ResultRow） */
export interface SearchResultItem {
  id: string;
  objectType: SearchObjectType;
  label: string;
  /** 行内编号等短说明（如 PRJ-008），11px 次文字 */
  description?: string;
  /** 行下方 11px 元数据行（项目 · 状态 · 参与者）；不参与分类（Figma 口径） */
  meta?: string;
  /** 选中后的导航目标（路由路径） */
  target: string;
}

/** 全局命令（CommandPalette 菜单项） */
export interface SearchCommand {
  id: string;
  label: string;
  /** navigate/create 渲染在主面板；action 渲染在「附加命令」面板（Figma Additional commands） */
  group: "navigate" | "create" | "action";
  icon?: "folder" | "gitBranch" | "plus" | "user" | "inbox";
  /** 选中后的导航目标（路由路径） */
  target: string;
}

/** 最近搜索（RecentSearches） */
export interface RecentSearch {
  id: string;
  label: string;
}

/** 异步搜索函数：rejected 即进入 Error 态 */
export type SearchFn = (query: string) => Promise<SearchResultItem[]>;

/** ResultRow 三态：选中（accent 着色面）与键盘焦点（2px accent 描边）是独立状态 */
export type SearchResultState = "default" | "selected" | "keyboardFocus";
