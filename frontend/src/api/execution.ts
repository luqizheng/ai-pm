import { http } from "@/shell/api";
import type { Paged } from "./types";

/**
 * AI 执行中心读模型：`delivery.task_run` ⋈ `delivery.task`（+ 失败摘要）。
 * 与 `api/types.ts` 分开维护，避免共享文件被多条会话同时修改。
 *
 * T-070 增补：列表补执行上下文列（run_kind / agent / executor / lease），
 * 新增单 Run 详情（租约 + 执行器 + 各仓基线；不读日志内容，只有定位信息）。
 */
export interface TaskRun {
  id: string;
  task_id: string;
  task_title: string;
  attempt_no: number;
  run_kind: string;
  status: string;
  outcome: string | null;
  agent_id: string | null;
  executor_kind: string | null;
  executor_session: string | null;
  lease_expires_at: string | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  failure_category: string | null;
  failure_reason: string | null;
}

export function listTaskRuns(
  params: {
    project_id?: string;
    task_id?: string;
    /** `delivery` / `review`；缺省不过滤（attempt_no 按 kind 分轨编号）。 */
    run_kind?: string;
    page?: number;
    size?: number;
  } = {}
) {
  return http.get<Paged<TaskRun>>("/v1/pm/task-runs", params);
}

/** 本 Run 锁定的一仓基线（`task_run_repository`）；`workspace_path` 即日志定位文本。 */
export interface RunRepository {
  repository_id: string;
  repository_name: string;
  base_ref: string;
  base_commit: string;
  write_access: boolean;
  workspace_path: string;
}

/** `GET /v1/pm/task-runs/{run_id}`：列表字段 + 阶段时间线 + 租约/执行器 + 基线区。 */
export interface TaskRunDetail extends TaskRun {
  project_id: string;
  stage: string | null;
  claimed_at: string | null;
  heartbeat_at: string | null;
  last_agent_heartbeat_at: string | null;
  deadline_at: string | null;
  release_reason: string | null;
  integration_status: string;
  review_verdict: string | null;
  repositories: RunRepository[];
}

export function getTaskRun(runId: string) {
  return http.get<TaskRunDetail>(`/v1/pm/task-runs/${runId}`);
}
