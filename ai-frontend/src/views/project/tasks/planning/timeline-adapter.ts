import type { DependencyGraph } from "@/api/task-plans";
import { shortId } from "../tasks-contract";

/**
 * S04-UI-06 Timeline · 视图适配（真实读模型 → 页面模型）。
 *
 * 数据源 = `GET /v1/pm/projects/{project_id}/dependency-graph`——节点自带
 * `planned_start_at` / `planned_end_at` / `estimate_hours`，边即 FS 依赖，
 * 因此甘特图、依赖连线、未排期托盘都能由真实读模型渲染。
 *
 * 纪律：
 * - **未排期**（无 planned_start_at）的任务进未排期托盘，不静默丢弃；
 * - 里程碑 / 特性分组 / 负责人 后端无来源 → 改为按**仓库**分组（节点真实
 *   字段 repository_name），负载分布同口径；
 * - 排期日期是规划估算，与实际执行时间无关。
 */

/** 时间线行（一个任务）。 */
export interface TimelineRowView {
  id: string;
  code: string;
  title: string;
  priority: string;
  status: string;
  repo: string;
  /** 分组键（仓库名；未绑定仓库为空串）。 */
  groupKey: string;
  /** 生效排期起点（ISO 时间戳）；null = 未排期。 */
  start: string | null;
  /** 生效排期终点（ISO 时间戳）；null = 未设终点。 */
  end: string | null;
  /** 工时估算（小时）；null = 未估。 */
  effortHours: number | null;
  critical: boolean;
  blocked: boolean;
}

/** 时间线分组（按仓库）。 */
export interface TimelineGroupView {
  id: string;
  repo: string;
  rows: TimelineRowView[];
}

/** 时间线视图模型。 */
export interface TimelineViewModel {
  rows: TimelineRowView[];
  groups: TimelineGroupView[];
  edges: Array<{ from: string; to: string }>;
  /** 网格起点（yyyy-mm-dd，UTC）。 */
  gridStart: string;
  /** 网格天数（覆盖全部已排期任务，至少 6 周）。 */
  gridDays: number;
  scheduledCount: number;
  unscheduledCount: number;
  criticalCount: number;
}

const DAY_MS = 86_400_000;
/** 最小窗口：6 周（与原页面比例尺一致）。 */
const MIN_WINDOW_DAYS = 42;

/** UTC 日期部分（yyyy-mm-dd）。 */
export function toDateKey(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return date.toISOString().slice(0, 10);
}

/** 日期差（天）。 */
export function diffDays(from: string, to: string): number {
  const a = new Date(`${from}T00:00:00Z`).getTime();
  const b = new Date(`${to}T00:00:00Z`).getTime();
  if (Number.isNaN(a) || Number.isNaN(b)) return 0;
  return Math.round((b - a) / DAY_MS);
}

/** 时间戳平移天数（保持时刻不变）。 */
export function shiftTimestamp(value: string, days: number): string {
  const time = new Date(value).getTime();
  if (Number.isNaN(time)) return value;
  return new Date(time + days * DAY_MS).toISOString();
}

/** 依赖图 → 时间线视图模型。 */
export function toTimelineView(graph: DependencyGraph): TimelineViewModel {
  const critical = new Set(graph.critical_path ?? []);

  const rows: TimelineRowView[] = (graph.nodes ?? []).map(node => ({
    id: node.id,
    code: shortId(node.id),
    title: node.title,
    priority: node.priority,
    status: node.status,
    repo: node.repository_name ?? "",
    groupKey: node.repository_name ?? "",
    start: node.planned_start_at,
    end: node.planned_end_at,
    effortHours: node.estimate_hours,
    critical: critical.has(node.id),
    blocked: node.status === "blocked"
  }));

  const groupMap = new Map<string, TimelineRowView[]>();
  for (const row of rows) {
    const bucket = groupMap.get(row.groupKey) ?? [];
    bucket.push(row);
    groupMap.set(row.groupKey, bucket);
  }
  const groups: TimelineGroupView[] = [...groupMap.entries()].map(
    ([repo, groupRows]) => ({
      id: repo || "__unbound__",
      repo,
      rows: groupRows
    })
  );

  const scheduled = rows.filter(row => row.start !== null);
  const startKeys = scheduled
    .map(row => toDateKey(row.start as string))
    .filter(key => key.length > 0)
    .sort();
  const gridStart = startKeys[0] ?? new Date().toISOString().slice(0, 10);

  const endKeys = scheduled
    .map(row => (row.end ? toDateKey(row.end) : toDateKey(row.start as string)))
    .filter(key => key.length > 0)
    .sort();
  const lastDay = endKeys[endKeys.length - 1] ?? gridStart;
  const gridDays = Math.max(MIN_WINDOW_DAYS, diffDays(gridStart, lastDay) + 1);

  return {
    rows,
    groups,
    edges: (graph.edges ?? []).map(edge => ({
      from: edge.from_task_id,
      to: edge.to_task_id
    })),
    gridStart,
    gridDays,
    scheduledCount: scheduled.length,
    unscheduledCount: rows.length - scheduled.length,
    criticalCount: rows.filter(row => row.critical).length
  };
}
