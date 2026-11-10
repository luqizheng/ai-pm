import {
  planTaskDrafts,
  type TaskPlanDetail,
  type TaskPlanEntry,
  type TaskPlanProgress
} from "@/api/task-plans";
import { shortId } from "../tasks-contract";

/**
 * S04-UI-02 Task Breakdown · 视图适配（真实读模型 → 页面模型）。
 *
 * 纪律：**只映射真有的字段**。计划修订正文里没有的段（拆解理由 / 来源追溯 /
 * 缺失工作 / 潜在重叠 / 风险 / 建议子任务）后端无来源，一律不渲染——不用
 * 演示文案填充。
 *
 * 字段来源：
 * - 计划本体 ← GET /v1/pm/projects/{id}/task-plans（列表项）；
 * - 当前修订 ← 计划详情 revisions 中 id == current_revision_id 的那条
 *   （缺 current_revision_id 时退回最新版本修订）；
 * - 任务草案 ← 当前修订正文 content.tasks[]（key / title / description /
 *   acceptance_criteria / depends_on / repository_ids / priority / change）；
 * - 正式任务 ← 计划详情 materialized_tasks（已 materialize 的正式任务）；
 * - 进度 ← GET /v1/pm/task-plans/{id}/progress（状态分布 + 阻塞清单）。
 */

/** 计划内任务草案（修订正文的一条）。 */
export interface BreakdownDraftView {
  key: string;
  title: string;
  description: string;
  priority: string;
  dependsOn: string[];
  acceptanceCriteria: string[];
  repositoryIds: string[];
  change: string;
}

/** 计划（聚合根）视图模型。 */
export interface BreakdownPlanView {
  id: string;
  code: string;
  title: string;
  status: string;
  baselineId: string;
  revisionId: string;
  revisionVersion: number;
  revisionState: string;
  revisionSource: string;
  instructions: string;
}

/** 已正式化的任务。 */
export interface BreakdownMaterializedView {
  taskId: string;
  code: string;
  title: string;
  status: string;
  planTaskKey: string;
}

/** 计划进度。 */
export interface BreakdownProgressView {
  totalTasks: number;
  plannedTasks: number;
  byStatus: Array<{ status: string; count: number }>;
  blocked: Array<{
    id: string;
    code: string;
    title: string;
    status: string;
    openUpstreams: number;
  }>;
}

function text(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean")
    return String(value);
  return "";
}

function stringList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value
    .map(item => (typeof item === "string" ? item : ""))
    .filter(item => item.length > 0);
}

/** 计划的当前修订（无 current_revision_id 时取版本号最大的那条）。 */
export function currentRevision(detail: TaskPlanDetail | null) {
  if (!detail) return null;
  const currentId = detail.plan.current_revision_id;
  if (currentId) {
    const found = detail.revisions.find(revision => revision.id === currentId);
    if (found) return found;
  }
  return detail.revisions[0] ?? null;
}

/** 计划列表项 → 页面模型（详情未加载时也可先渲染）。 */
export function toBreakdownPlan(
  entry: TaskPlanEntry,
  detail: TaskPlanDetail | null = null
): BreakdownPlanView {
  const revision = detail ? currentRevision(detail) : null;
  return {
    id: entry.id,
    code: entry.code,
    title: entry.title,
    status: entry.status,
    baselineId: entry.source_baseline_id ?? "",
    revisionId: revision?.id ?? "",
    revisionVersion: revision?.version ?? 0,
    revisionState: revision?.state ?? "",
    revisionSource: revision?.source ?? "",
    instructions: revision?.instructions ?? ""
  };
}

/** 当前修订正文的任务草案 → 页面模型。 */
export function toBreakdownDrafts(
  detail: TaskPlanDetail | null
): BreakdownDraftView[] {
  const revision = currentRevision(detail);
  return planTaskDrafts(revision).map(
    (draft): BreakdownDraftView => ({
      key: draft.key,
      title: draft.title,
      description: text(draft.description),
      priority: draft.priority ?? "",
      dependsOn: stringList(draft.depends_on),
      acceptanceCriteria: stringList(draft.acceptance_criteria),
      repositoryIds: stringList(draft.repository_ids),
      change: text(draft.change)
    })
  );
}

/** 已正式化任务 → 页面模型。 */
export function toBreakdownMaterialized(
  detail: TaskPlanDetail | null
): BreakdownMaterializedView[] {
  if (!detail) return [];
  return detail.materialized_tasks.map(
    (task): BreakdownMaterializedView => ({
      taskId: task.task_id,
      code: shortId(task.task_id),
      title: task.title,
      status: task.status,
      planTaskKey: task.plan_task_key ?? ""
    })
  );
}

/** 计划进度 → 页面模型。 */
export function toBreakdownProgress(
  progress: TaskPlanProgress | null
): BreakdownProgressView {
  if (!progress) {
    return { totalTasks: 0, plannedTasks: 0, byStatus: [], blocked: [] };
  }
  const byStatus = Object.entries(progress.by_status ?? {}).map(
    ([status, count]) => ({ status, count })
  );
  const blocked = (progress.blocked ?? []).map(row => {
    const record = (row ?? {}) as Record<string, unknown>;
    const id = text(record.id);
    return {
      id,
      code: shortId(id),
      title: text(record.title),
      status: text(record.status),
      openUpstreams:
        typeof record.open_upstreams === "number" ? record.open_upstreams : 0
    };
  });
  return {
    totalTasks: progress.total_tasks ?? 0,
    plannedTasks: progress.planned_tasks ?? 0,
    byStatus,
    blocked
  };
}
