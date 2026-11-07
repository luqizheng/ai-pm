/**
 * Global/Search 演示数据（骨架阶段）。
 *
 * 样本与 Figma「01 — Global Search」/「02 — Search Results」一致：
 * 查询「atlas」返回六类对象各一条；最近搜索三条。
 * 命令（Open project / Create project …）是 UI 文案，走 i18n，
 * 在 WorkspaceShell 里组装，不落在本文件。
 *
 * 接入真实数据源时整体替换（demoSearchFn → 全局检索接口）。
 */
import type { RecentSearch, SearchFn, SearchResultItem } from "./types";

/** 最近搜索（Figma Recent searches：Atlas Platform / REQ-142 / atlas-api） */
export const demoRecentSearches: RecentSearch[] = [
  { id: "recent-atlas-platform", label: "Atlas Platform" },
  { id: "recent-req-142", label: "REQ-142 · Session timeout" },
  { id: "recent-repo-atlas-api", label: "Repository · atlas-api" }
];

/** 演示搜索索引（六类各一条，Figma Populated 样本） */
export const demoSearchIndex: SearchResultItem[] = [
  {
    id: "search-project-atlas",
    objectType: "project",
    label: "Atlas Platform",
    description: "PRJ-008",
    meta: "Acme Research · Contributor · Open object →",
    target: "/project/settings/repositories"
  },
  {
    id: "search-repo-atlas-api",
    objectType: "repository",
    label: "atlas-api",
    description: "PRJ-008",
    meta: "Atlas Platform · Private repository · Open object →",
    target: "/project/settings/repositories"
  },
  {
    id: "search-req-142",
    objectType: "requirement",
    label: "Session timeout",
    description: "REQ-142",
    meta: "Atlas Platform · REQ-142 · In review · Open object →",
    target: "/project/requirements"
  },
  {
    id: "search-task-318",
    objectType: "task",
    label: "Atlas Platform",
    description: "TASK-318",
    meta: "Atlas Platform · TASK-318 · Maya Chen · Open object →",
    target: "/project/execution"
  },
  {
    id: "search-run-089",
    objectType: "aiRun",
    label: "Atlas analysis",
    description: "RUN-089",
    meta: "Atlas Platform · RUN-089 · Analysis completed · Open object →",
    target: "/project/execution"
  },
  {
    id: "search-doc-024",
    objectType: "document",
    label: "Atlas Platform guide",
    description: "DOC-024",
    meta: "Atlas Platform · DOC-024 · Updated today · Open object →",
    target: "/project/knowledge"
  }
];

/** 演示搜索：按 label / description / meta 包含匹配，短延时跑出 Searching 态 */
export const demoSearchFn: SearchFn = query => {
  const q = query.trim().toLowerCase();
  return new Promise(resolve => {
    setTimeout(() => {
      if (!q) {
        resolve([]);
        return;
      }
      resolve(
        demoSearchIndex.filter(item =>
          [item.label, item.description ?? "", item.meta ?? ""]
            .join(" ")
            .toLowerCase()
            .includes(q)
        )
      );
    }, 250);
  });
};
