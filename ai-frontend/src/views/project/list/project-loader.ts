import { listProjects } from "@/api/projects";
import { listRepositories } from "@/api/repositories";
import type { Project, Repository } from "@/api/types";
import type { ProjectEntry, ProjectRepoSummary } from "./project-entry";

/**
 * Projects List 真实数据源（发布构建走这里；fixtures 只在 VITE_USE_MOCK=1 时加载）。
 *
 * 列表行以 `GET /v1/pm/projects` 为锚，仓库摘要逐项目读
 * `GET /v1/pm/projects/{id}/repositories` 聚合（仓库管理属项目设置域，不在本页展开）。
 */

const EMPTY_REPO_SUMMARY: ProjectRepoSummary = {
  count: 0,
  primary_name: null,
  has_unreachable: false
};

/** RFC3339 → "YYYY-MM-DD HH:mm"（列表行展示口径） */
function formatTimestamp(value: string): string {
  if (!value) return "";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(
    parsed.getDate()
  )} ${pad(parsed.getHours())}:${pad(parsed.getMinutes())}`;
}

/** core.project.status → 列表生命周期（placeholder 属进行中，不单独成档） */
function lifecycleOf(status: string): ProjectEntry["status"] {
  if (status === "archived") return "archived";
  if (status === "placeholder") return "placeholder";
  return "active";
}

/** 仓库摘要聚合：归档仓库不计入数量；不可达仓库需要人工关注 */
function summarize(repositories: Repository[]): ProjectRepoSummary {
  const active = repositories.filter(repo => repo.status !== "archived");
  const primary = active.find(repo => repo.role === "main");
  return {
    count: active.length,
    primary_name: primary?.name ?? null,
    has_unreachable: repositories.some(repo => repo.status === "unreachable")
  };
}

async function loadRepoSummary(projectId: string): Promise<ProjectRepoSummary> {
  try {
    const { items } = await listRepositories(projectId);
    return summarize(items);
  } catch {
    // 单个仓库摘要失败不拖垮整页；宁可显示零仓库也不虚构
    return EMPTY_REPO_SUMMARY;
  }
}

function toEntry(project: Project, repositories: ProjectRepoSummary) {
  return {
    id: project.id,
    code: project.code,
    name: project.name,
    description: project.description ?? "",
    status: lifecycleOf(project.status),
    customer_name:
      project.customer_status === "placeholder" ? null : project.customer_name,
    repositories,
    updated_at: formatTimestamp(project.updated_at),
    created_at: formatTimestamp(project.created_at)
  } satisfies ProjectEntry;
}

export async function loadProjectEntriesFromApi(): Promise<ProjectEntry[]> {
  const page = await listProjects({ size: 100 });
  const summaries = await Promise.all(
    page.items.map(project => loadRepoSummary(project.id))
  );
  return page.items.map((project, index) =>
    toEntry(project, summaries[index] ?? EMPTY_REPO_SUMMARY)
  );
}
