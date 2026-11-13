import type { Task } from "@/api/types";

/**
 * S04 任务域视图契约（docs/scenarios/s04/ui，2026-11-10）。
 *
 * 契约裁决：状态/优先级词汇 = 后端真实枚举（delivery.task 13 态、
 * priority 四级，crates/pm-domain task_transition），设计稿草案里的
 * P0–P3 / Pending Review / Approved 等词汇不进 UI。
 *
 * 色调映射走全局语义 token（prism 门禁禁止裸色值）：
 *   ready → --ws-color-status-ready；blocked / awaiting_decision /
 *   changes_requested → warning；failed → danger；completed → success；
 *   执行通道（claimed/running/submitted/reviewing）→ info；
 *   草稿 → neutral；终态作废 → muted。
 */

export type TaskStatusTone =
  | "neutral"
  | "info"
  | "ready"
  | "warning"
  | "danger"
  | "success"
  | "muted";

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
  title: string;
  description: string | null;
  status: string;
  priority: string;
  assigneeId: string | null;
  milestoneId: string | null;
  revision: number;
  updatedAt: string;
}

export function toTaskRow(task: Task): TaskRowModel {
  return {
    id: task.id,
    title: task.title,
    description: task.description,
    status: task.status,
    priority: task.priority,
    assigneeId: task.assignee_id,
    milestoneId: task.milestone_id,
    revision: task.revision,
    updatedAt: task.updated_at
  };
}
