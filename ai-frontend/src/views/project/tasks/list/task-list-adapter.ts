import type { Task, TaskProductRef } from "@/api/types";
import {
  aggregateStatusLabel,
  aggregateStatusTone,
  executionDisplay,
  formatDue,
  priorityGrade,
  schedulingCondition,
  statusTone,
  type ExecutionDisplay,
  type SchedulingCondition,
  type TaskStatusTone
} from "../tasks-contract";

/**
 * PA-07 任务列表行模型（Figma 179:3603 A）。
 *
 * 只映射后端真实返回的字段（task-detail-adapter 纪律）：code/planned_end_at/
 * products/latest_run/deps 计数随 0050 列表读面返回，无来源的列一律不伪造
 * （Harness 裁决：无数据源不显示）。
 */
export interface TaskListRow {
  id: string;
  code: string;
  title: string;
  status: string;
  statusLabel: string;
  statusTone: TaskStatusTone;
  priority: string;
  grade: "P0" | "P1" | "P2" | "P3";
  due: string;
  products: TaskProductRef[];
  productNames: string;
  execution: ExecutionDisplay;
  condition: SchedulingCondition;
  updatedAt: string;
}

export function toListRow(task: Task): TaskListRow {
  return {
    id: task.id,
    code: task.code,
    title: task.title,
    status: task.status,
    statusLabel: aggregateStatusLabel(task.status),
    statusTone: aggregateStatusTone(task.status),
    priority: task.priority,
    grade: priorityGrade(task.priority),
    due: formatDue(task.planned_end_at),
    products: task.products ?? [],
    productNames: (task.products ?? []).map(p => p.name).join("\n"),
    execution: executionDisplay(task.latest_run),
    condition: schedulingCondition(task),
    updatedAt: task.updated_at
  };
}

/** Dependency 筛选档（设计稿 Dependency: All 下拉） */
export const DEPENDENCY_FILTERS = ["running", "blocked_product", "waiting", "ready"] as const;
export type DependencyFilterKey = (typeof DEPENDENCY_FILTERS)[number];

export interface TaskListFilters {
  product: string;
  status: string;
  dependency: string;
  search: string;
}

export function filterRows(rows: TaskListRow[], filters: TaskListFilters): TaskListRow[] {
  const needle = filters.search.trim().toLowerCase();
  return rows.filter(row => {
    if (filters.product !== "__all__" && !row.products.some(p => p.id === filters.product)) {
      return false;
    }
    if (filters.status !== "__all__" && row.status !== filters.status) {
      return false;
    }
    if (filters.dependency !== "__all__" && row.condition.key !== filters.dependency) {
      return false;
    }
    if (needle && !`${row.code} ${row.title}`.toLowerCase().includes(needle)) {
      return false;
    }
    return true;
  });
}

/** 筛选器下拉的产品选项（列表行内出现过的关联产品聚合去重） */
export function productOptions(rows: TaskListRow[]): Array<{ value: string; label: string }> {
  const seen = new Map<string, string>();
  for (const row of rows) {
    for (const product of row.products) {
      if (!seen.has(product.id)) seen.set(product.id, product.name);
    }
  }
  return [...seen.entries()]
    .sort((a, b) => a[1].localeCompare(b[1]))
    .map(([value, label]) => ({ value, label }));
}

/** 右栏 RELATED PRODUCTS：项目关联产品（列表页拉一次） */
export interface RelatedProduct {
  id: string;
  name: string;
  status: string;
  taskCount: number;
}
