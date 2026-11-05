/**
 * Project List 展示契约（mock 先行）。
 *
 * 页面读模型：以 `src/api/types.ts` 的 `Project`（core.project）与
 * `Repository`（integration.project_repository）为锚——`ProjectEntry` 对应
 * Project 行本体，仓库信息在读取侧预聚合为 `ProjectRepoSummary` 摘要
 * （列表页只展示「有多少仓库 / 主仓库是谁」，仓库管理属项目域，不在本页展开）。
 * 后端落地时以本文件为读模型参照。
 *
 * 红线（S01-02/S01-03 裁决 + Projects List 设计稿 §13）：
 *   - Project 生命周期仅 active / archived；placeholder（未归类项目）仍属
 *     进行中，不作为独立生命周期档；初始化进度不是 Project 状态，不在本页出现；
 *   - 一个 Project 可关联多个 Repository，Primary（role=main）可空——
 *     展示必须兼容「零仓库」「多仓库」「无主仓库」；
 *   - 不引入 Workspace 概念，不把仓库管理做成页面视觉焦点。
 */

/** Project 生命周期状态（types.ts Project.status 注释口径） */
export type ProjectLifecycle = "active" | "archived" | "placeholder";

/** 项目仓库摘要（由 integration.project_repository 聚合） */
export interface ProjectRepoSummary {
  /** 仓库总数（可为 0） */
  count: number;
  /** 主仓库（role=main）名称；Primary 可空（S01-03 裁决） */
  primary_name: string | null;
  /** 存在核验失败（status=unreachable）的仓库，需人工处理 */
  has_unreachable: boolean;
}

/** 项目列表行读模型（锚定 types.ts 的 Project） */
export interface ProjectEntry {
  id: string;
  /** 项目编码（types.ts Project.code） */
  code: string;
  name: string;
  description: string;
  status: ProjectLifecycle;
  /** 冗余客户显示名；未归类（placeholder）时为 null */
  customer_name: string | null;
  repositories: ProjectRepoSummary;
  /** 展示格式 "YYYY-MM-DD HH:mm" */
  updated_at: string;
  created_at: string;
}

/** 「最近活跃」窗口：updated_at 落在最近 7 天内 */
export const RECENT_WINDOW_DAYS = 7;

/** 解析展示时间戳（"YYYY-MM-DD HH:mm" → epoch ms；无法解析返回 0） */
export function parseTimestamp(value: string): number {
  const normalized = value.includes("T") ? value : value.replace(" ", "T");
  const parsed = Date.parse(normalized);
  return Number.isNaN(parsed) ? 0 : parsed;
}

/** 是否最近活跃（RECENT_WINDOW_DAYS 内有更新） */
export function isRecentlyActive(entry: ProjectEntry, now: number): boolean {
  const updated = parseTimestamp(entry.updated_at);
  return (
    updated > 0 && now - updated <= RECENT_WINDOW_DAYS * 24 * 60 * 60 * 1000
  );
}

/** 行/卡片的操作种类（域支持的操作；Edit 无落地页不提供） */
export type ProjectActionKind = "open" | "archive" | "restore" | "delete";

/** 数据加载器签名（页根缺省走 fixtures；接真 API 时替换实现） */
export type ProjectsLoader = () => Promise<ProjectEntry[]>;
