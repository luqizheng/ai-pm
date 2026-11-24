import type { Task, TaskProductRef } from "@/api/types";

/**
 * S04 任务域视图契约（docs/scenarios/s04/ui，2026-11-10；PA-07 增补 2026-11-21）。
 *
 * 契约裁决：状态词汇 = 后端真实枚举（delivery.task 13 态，crates/pm-domain
 * task_transition）。PA-07 设计稿（Figma 179:3603）引入两个展示层词表，均为
 * **后端事实的聚合/派生**，不新增领域语义：
 *   1. 五态聚合 Not started / In progress / Completed / Failed / Cancelled
 *      （Figma「PA-07 / Task Status Indicator」组件说明原文：Task lifecycle
 *      only——依赖阻塞/产品暂停不改变任务生命周期状态）；
 *   2. Scheduling condition 四态（Waiting / Ready / Running / Blocked by
 *      Product Suspension）——纯读侧派生，与 13 态状态机并行展示。
 * 优先级 P0–P3 = urgent/high/normal/low 的展示映射（数量一致）。
 *
 * 色调映射走全局语义 token（prism 门禁禁止裸色值）：
 *   ready → --ws-color-status-ready；blocked / awaiting_decision /
 *   changes_requested → warning；failed → danger；completed → success；
 *   执行通道（claimed/running/submitted/reviewing）→ info；
 *   草稿 → neutral；终态作废 → muted；
 *   PA-07 新增 scheduled（Ready to Start）→ --ws-color-status-scheduled。
 */

export type TaskStatusTone =
  | "neutral"
  | "info"
  | "ready"
  | "warning"
  | "danger"
  | "success"
  | "muted"
  | "scheduled";

export interface TaskStatusMeta {
  value: string;
  tone: TaskStatusTone;
}

/** 13 态全量（顺序 = 展示序：规划态 → 执行通道 → 终态） */
export const TASK_STATUSES: TaskStatusMeta[] = [
  { value: "draft", tone: "neutral" },
  { value: "ready", tone: "ready" },
  { value: "blocked", tone: "warning" },
  { value: "claimed", tone: "info" },
  { value: "running", tone: "info" },
  { value: "awaiting_decision", tone: "warning" },
  { value: "submitted", tone: "info" },
  { value: "reviewing", tone: "info" },
  { value: "changes_requested", tone: "warning" },
  { value: "completed", tone: "success" },
  { value: "failed", tone: "danger" },
  { value: "cancelled", tone: "muted" },
  { value: "superseded", tone: "muted" }
];

export type TaskStatus = (typeof TASK_STATUSES)[number]["value"];

export const TASK_PRIORITIES = ["urgent", "high", "normal", "low"] as const;

export type TaskPriority = (typeof TASK_PRIORITIES)[number];

export function priorityTone(priority: string): TaskStatusTone {
  switch (priority) {
    case "urgent":
      return "danger";
    case "high":
      return "warning";
    case "low":
      return "muted";
    default:
      return "neutral";
  }
}

export function statusTone(status: string): TaskStatusTone {
  return TASK_STATUSES.find(meta => meta.value === status)?.tone ?? "neutral";
}

// ───────────────────── PA-07 展示层词表（Figma 179:3603，均为派生）

/** PA-07 五态聚合（Task Status Indicator 组件：lifecycle only） */
export const TASK_STATUS_AGGREGATES = [
  {
    key: "not_started",
    label: "Not started",
    statuses: ["draft", "ready", "blocked"]
  },
  {
    key: "in_progress",
    label: "In progress",
    statuses: [
      "claimed",
      "running",
      "awaiting_decision",
      "submitted",
      "reviewing",
      "changes_requested"
    ]
  },
  { key: "completed", label: "Completed", statuses: ["completed"] },
  { key: "failed", label: "Failed", statuses: ["failed"] },
  {
    key: "cancelled",
    label: "Cancelled",
    statuses: ["cancelled", "superseded"]
  }
] as const;

export type TaskStatusAggregateKey =
  (typeof TASK_STATUS_AGGREGATES)[number]["key"];

export function aggregateStatus(status: string): TaskStatusAggregateKey {
  return (
    TASK_STATUS_AGGREGATES.find(agg =>
      (agg.statuses as readonly string[]).includes(status)
    )?.key ?? "not_started"
  );
}

export function aggregateStatusLabel(status: string): string {
  const key = aggregateStatus(status);
  return (
    TASK_STATUS_AGGREGATES.find(agg => agg.key === key)?.label ?? "Not started"
  );
}

export function aggregateStatusTone(status: string): TaskStatusTone {
  switch (aggregateStatus(status)) {
    case "completed":
      return "success";
    case "failed":
      return "danger";
    case "cancelled":
      return "muted";
    case "in_progress":
      return "info";
    default:
      return "neutral";
  }
}

/** P0–P3 展示映射：urgent/high/normal/low → P0/P1/P2/P3（数量一致，纯展示） */
export function priorityGrade(priority: string): "P0" | "P1" | "P2" | "P3" {
  switch (priority) {
    case "urgent":
      return "P0";
    case "high":
      return "P1";
    case "low":
      return "P3";
    default:
      return "P2";
  }
}

/** Due 列文本（planned_end_at；无排期空串）。设计稿口径「Due 23 Nov」。 */
export function formatDue(
  value: string | null | undefined,
  today = new Date()
): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const startOf = (d: Date) =>
    new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round(
    (startOf(date) - startOf(today)) / (24 * 60 * 60 * 1000)
  );
  const label = date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric"
  });
  if (days === 0) return `Due today`;
  if (days < 0) return `Overdue · ${label}`;
  return `Due ${label}`;
}

/** EXECUTION 列（PA-07）：无 run → No run；有 run → Run #N + 状态副行 */
export interface ExecutionDisplay {
  label: string;
  detail: string | null;
}

export function executionDisplay(
  latestRun: Task["latest_run"]
): ExecutionDisplay {
  if (!latestRun) return { label: "No run", detail: null };
  const active = ["queued", "claimed", "running"].includes(latestRun.status);
  return {
    label: `Run #${latestRun.attempt_no}`,
    detail: active ? `${latestRun.status} · Active` : latestRun.status
  };
}

/** SCHEDULING CONDITION 列派生（设计稿四态 + 终态；纯读侧，不改任务状态） */
export interface SchedulingCondition {
  key: "running" | "blocked_product" | "waiting" | "ready" | "closed";
  label: string;
  detail: string | null;
  tone: TaskStatusTone;
}

const ACTIVE_RUN_STATUSES = ["queued", "claimed", "running"];

export function schedulingCondition(row: {
  status: string;
  product_blocked: boolean;
  deps_total: number;
  deps_met: number;
  latest_run: Task["latest_run"];
  products?: TaskProductRef[] | null;
}): SchedulingCondition {
  // 终态：调度条件随生命周期关闭（设计稿尾注：conditions never replace Task status）。
  if (["completed", "failed", "cancelled", "superseded"].includes(row.status)) {
    return {
      key: "closed",
      label: "Closed",
      detail: aggregateStatusLabel(row.status),
      tone: aggregateStatusTone(row.status)
    };
  }
  // 有活跃 run：执行中（attempt 递增，RN-xx 语义）。
  if (row.latest_run && ACTIVE_RUN_STATUSES.includes(row.latest_run.status)) {
    return {
      key: "running",
      label: "Running",
      detail: `Run #${row.latest_run.attempt_no} · Active`,
      tone: "info"
    };
  }
  // 产品门禁拒绝（suspended/archived 关联产品）：红字阻塞。设计稿明确不改任务
  // 状态，只作为调度条件展示。
  if (row.product_blocked) {
    const gated = (row.products ?? [])
      .filter(p => p.status === "suspended" || p.status === "archived")
      .map(p => p.name);
    return {
      key: "blocked_product",
      label: "Blocked by Product Suspension",
      detail: gated.length
        ? `${gated.join(", ")} · Development restricted`
        : null,
      tone: "danger"
    };
  }
  const total = row.deps_total;
  const met = Math.min(row.deps_met, total);
  if (total > 0 && met < total) {
    return {
      key: "waiting",
      label: "Waiting for Dependencies",
      detail: `${met}/${total} met`,
      tone: "neutral"
    };
  }
  return {
    key: "ready",
    label: "Ready to Start",
    detail: total > 0 ? `${met}/${total} met` : "No predecessors",
    tone: "scheduled"
  };
}

/** 看板列：规划态 / 执行通道 / 终态聚合（13 态不适合平铺成 13 列） */
export const KANBAN_LANES: Array<{
  key: "draft" | "ready" | "blocked" | "active" | "closed";
  statuses: string[];
}> = [
  { key: "draft", statuses: ["draft"] },
  { key: "ready", statuses: ["ready"] },
  { key: "blocked", statuses: ["blocked"] },
  {
    key: "active",
    statuses: [
      "claimed",
      "running",
      "awaiting_decision",
      "submitted",
      "reviewing",
      "changes_requested"
    ]
  },
  {
    key: "closed",
    statuses: ["completed", "failed", "cancelled", "superseded"]
  }
];

/** UUID 短形（列表/看板展示用；完整 UUID 仅在表单与依赖输入出现） */
export function shortId(id: string | null | undefined): string {
  if (!id) return "";
  return id.length > 8 ? id.slice(0, 8) : id;
}

export function formatDateTime(value: string | null | undefined): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  });
}

/** 任务池行模型：DTO + 展示辅助 */
export interface TaskRowModel {
  id: string;
  code: string;
  title: string;
  description: string | null;
  status: string;
  priority: string;
  assigneeId: string | null;
  milestoneId: string | null;
  revision: number;
  updatedAt: string;
  /** 最新一次 run 的状态（null = 从未执行）；执行类型/执行中判定的展示源 */
  runStatus: string | null;
}

export function toTaskRow(task: Task): TaskRowModel {
  return {
    id: task.id,
    code: task.code,
    title: task.title,
    description: task.description,
    status: task.status,
    priority: task.priority,
    assigneeId: task.assignee_id,
    milestoneId: task.milestone_id,
    revision: task.revision,
    updatedAt: task.updated_at,
    runStatus: task.latest_run?.status ?? null
  };
}
