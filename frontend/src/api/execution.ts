import { http } from "@/shell/api";
import type { Paged } from "./types";

/**
 * AI 执行中心读模型：`delivery.task_run` ⋈ `delivery.task`（+ 失败摘要）。
 * 与 `api/types.ts` 分开维护，避免共享文件被多条会话同时修改。
 */
export interface TaskRun {
  id: string;
  task_id: string;
  task_title: string;
  attempt_no: number;
  status: string;
  outcome: string | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  failure_category: string | null;
  failure_reason: string | null;
}

export function listTaskRuns(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<TaskRun>>("/v1/pm/task-runs", params);
}