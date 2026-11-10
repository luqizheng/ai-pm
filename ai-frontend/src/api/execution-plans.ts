import { http } from "@/api/base";

/**
 * S05 · 执行与协作读模型（pm-api `pm_runs.rs` / `pm_execution.rs`）。
 *
 * 领域语义（docs/ui/s05 页面结构 + 总体设计 §3/§5/§6）：
 * - **Task ≠ Run ≠ Agent ≠ Delivery ≠ Review**：五层分列，不可混排。
 * - **Run 七态**：queued / claimed / running / succeeded / failed / timed_out /
 *   cancelled；attempt_no 按 kind（delivery / review）分轨编号。
 * - **完成三态分离**：Agent 声称完成（Run succeeded / Delivery submitted）
 *   ≠ AI-PM 检查通过（review verdict）≠ 人工正式验收（Task completed）。
 * - **waiting reason 五值**：needs_info / needs_decision / needs_approval /
 *   external_dependency / no_executor_available（库级 CHECK，原样读出）。
 * - **任务级一次性授权 ≠ ACP-07 Run 级 Deadline 重授权**：后者是 Run 级事实。
 * - **review verdict 三值**：approved / changes_requested / inconclusive。
 */

/** Run 七态。 */
export type RunStatus =
  | "queued"
  | "claimed"
  | "running"
  | "succeeded"
  | "failed"
  | "timed_out"
  | "cancelled";

/** Run kind（attempt_no 分轨）。 */
export type RunKind = "delivery" | "review";

/** 等待原因五值。 */
export type WaitingReason =
  | "needs_info"
  | "needs_decision"
  | "needs_approval"
  | "external_dependency"
  | "no_executor_available";

/** 交互原因（interaction 侧四值，不含 no_executor_available）。 */
export type InteractionReason =
  | "needs_info"
  | "needs_decision"
  | "needs_approval"
  | "external_dependency";

/** 评审裁决三值。 */
export type ReviewVerdict = "approved" | "changes_requested" | "inconclusive";

/** 设备就绪链六态。 */
export type DeviceReadiness =
  | "installed"
  | "connected"
  | "authenticated"
  | "ready"
  | "degraded"
  | "failed";

/** Run 列表行。 */
export interface TaskRun {
  id: string;
  task_id: string;
  task_title: string;
  attempt_no: number;
  run_kind: RunKind | string;
  status: RunStatus | string;
  outcome: string | null;
  agent_id: string | null;
  executor_kind: string | null;
  executor_session: string | null;
  lease_expires_at: string | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  /** 评审裁决（三值；未评审为 null）。 */
  review_verdict: ReviewVerdict | string | null;
  failure_category: string | null;
  failure_reason: string | null;
}

/** Run 详情（租约 / 执行器 / 阶段时间线 / 仓基线）。 */
export interface TaskRunDetail extends TaskRun {
  project_id: string;
  stage: string | null;
  claimed_at: string | null;
  heartbeat_at: string | null;
  last_agent_heartbeat_at: string | null;
  deadline_at: string | null;
  release_reason: string | null;
  integration_status: string;
  repositories: RunRepository[];
}

/** Run 绑定的仓基线。 */
export interface RunRepository {
  repository_id: string;
  repository_name: string;
  base_ref: string;
  base_commit: string;
  write_access: boolean;
  workspace_path: string;
}

/** Run 事件行（delivery.task_run_event 八类事件）。 */
export interface RunEvent {
  id: string;
  run_id: string;
  task_id: string;
  event_type: string;
  lease_generation: number;
  client_id: string | null;
  occurred_at: string;
  received_at: string;
  detail: Record<string, unknown> | null;
}

/** 等待行（活跃 waiting）。 */
export interface WaitingItem {
  id: string;
  task_id: string;
  task_title: string | null;
  task_status: string | null;
  task_priority: string | null;
  reason: WaitingReason | string;
  questions: unknown[];
  requested_from: string | null;
  since: string;
  deadline: string | null;
}

/** 待答交互行。 */
export interface InteractionItem {
  id: string;
  task_id: string;
  task_title: string | null;
  run_id: string;
  reason: InteractionReason | string;
  title: string;
  /** 问题原文（人工答复的对象）。 */
  question: string;
  options: unknown;
  required_response: boolean;
  deadline_at: string;
  created_at: string;
}

/** 待裁决审批行。 */
export interface PendingApprovalItem {
  id: string;
  source: "review" | "design" | string;
  kind: string;
  task_id: string | null;
  task_title: string | null;
  status: string;
  created_at: string;
}

/** 需要人工处理队列。 */
export interface HumanAttentionQueue {
  waiting: WaitingItem[];
  interactions: InteractionItem[];
  approvals: PendingApprovalItem[];
}

/** 执行态势概览。 */
export interface ExecutionOverview {
  active_run_count: number;
  waiting_count: number;
  pending_interaction_count: number;
  pending_approval_count: number;
  failed_run_count: number;
  awaiting_review_count: number;
  total_run_count: number;
  succeeded_run_count: number;
  success_rate: number | null;
  average_duration_seconds: number | null;
  device_ready_count: number;
  device_total_count: number;
  waiting_by_reason: Array<[string, number]>;
}

/** ACP-07 重授权行（Run 级）。 */
export interface RunAuthorization {
  id: string;
  run_id: string;
  interaction_id: string | null;
  authorized_by: string;
  authorized_by_name: string | null;
  reason: string;
  new_deadline_at: string;
  granted_at: string;
  expires_at: string;
  revoked_at: string | null;
}

/** Worker 命令行。 */
export interface WorkerCommand {
  id: string;
  command_type: string;
  worker_id: string;
  task_id: string;
  run_id: string;
  payload: Record<string, unknown>;
  status: string;
  claim_count: number;
  issued_at: string;
  delivered_at: string | null;
  acked_at: string | null;
  failed_at: string | null;
  last_error: string | null;
}

/** Continuation 行。 */
export interface RunContinuation {
  id: string;
  task_id: string;
  run_id: string;
  origin_interaction_id: string;
  generation: number;
  status: string;
  frozen_at: string | null;
  created_at: string;
  started_at: string | null;
  closed_at: string | null;
}

/** Run 级逐仓交付行。 */
export interface RunCommit {
  id: string;
  repository_id: string;
  repository_name: string | null;
  commit_sha: string;
  branch: string | null;
  changed_files: number;
  test_evidence: Record<string, unknown>;
  commit_author: string | null;
  commit_authored_at: string | null;
  created_at: string;
}

/** 协作图节点。 */
export interface CollaborationNode {
  id: string;
  title: string;
  status: string;
  priority: string;
  run_id: string | null;
  attempt_no: number | null;
  run_status: string | null;
  run_kind: string | null;
  agent_id: string | null;
  agent_name: string | null;
  open_upstream_count: number;
  planned_start_at: string | null;
  planned_end_at: string | null;
  estimate_hours: number | null;
}

/** 协作图边。 */
export interface CollaborationEdge {
  from_task_id: string;
  to_task_id: string;
}

/** 协作图里的 Agent 行。 */
export interface CollaborationAgent {
  agent_id: string;
  agent_name: string | null;
  run_count: number;
  active_run_count: number;
}

/** 多 Agent 协作图。 */
export interface CollaborationGraph {
  nodes: CollaborationNode[];
  edges: CollaborationEdge[];
  agents: CollaborationAgent[];
}

/** Agent / 设备名册行。 */
export interface AgentRosterRow {
  kind: "principal" | "device" | string;
  id: string;
  name: string;
  readiness: DeviceReadiness | string | null;
  executor_kind: string | null;
  platform: string | null;
  active_run_count: number;
  total_run_count: number;
}

function items<T>(payload: { items?: T[] } | null | undefined): T[] {
  return payload?.items ?? [];
}

/** `GET /v1/pm/task-runs`：Run 列表（可按项目 / 任务 / kind 过滤）。 */
export async function listTaskRuns(
  params: {
    project_id?: string;
    task_id?: string;
    run_kind?: string;
    page?: number;
    size?: number;
  } = {}
) {
  const payload = await http.get<{ items: TaskRun[]; total?: number }>(
    "/v1/pm/task-runs",
    params
  );
  return { items: items(payload), total: payload.total ?? 0 };
}

/** `GET /v1/pm/task-runs/{run_id}`：Run 详情。 */
export function getTaskRun(runId: string) {
  return http.get<TaskRunDetail>(`/v1/pm/task-runs/${runId}`);
}

/** `GET /v1/pm/task-runs/{run_id}/events`：Run 事件时间线。 */
export async function listRunEvents(runId: string) {
  const payload = await http.get<{ items: RunEvent[] }>(
    `/v1/pm/task-runs/${runId}/events`
  );
  return items(payload);
}

/** `GET /v1/pm/projects/{project_id}/waiting`：需要人工处理队列。 */
export function getHumanAttentionQueue(projectId: string) {
  return http.get<HumanAttentionQueue>(`/v1/pm/projects/${projectId}/waiting`);
}

/** `GET /v1/pm/projects/{project_id}/execution-overview`：执行态势概览。 */
export function getExecutionOverview(projectId: string) {
  return http.get<ExecutionOverview>(
    `/v1/pm/projects/${projectId}/execution-overview`
  );
}

/** `GET /v1/pm/projects/{project_id}/run-events`：项目级执行活动流。 */
export async function listProjectRunEvents(projectId: string, limit = 100) {
  const payload = await http.get<{ items: RunEvent[] }>(
    `/v1/pm/projects/${projectId}/run-events`,
    { limit }
  );
  return items(payload);
}

/** `GET /v1/pm/projects/{project_id}/agents`：Agent / 设备名册。 */
export async function listAgents(projectId: string) {
  const payload = await http.get<{ items: AgentRosterRow[] }>(
    `/v1/pm/projects/${projectId}/agents`
  );
  return items(payload);
}

/** `GET /v1/pm/tasks/{task_id}/collaboration`：多 Agent 协作图。 */
export function getCollaborationGraph(taskId: string) {
  return http.get<CollaborationGraph>(`/v1/pm/tasks/${taskId}/collaboration`);
}

/** `GET /v1/pm/task-runs/{run_id}/execution-authorization`：ACP-07 重授权台账。 */
export async function listRunAuthorizations(runId: string) {
  const payload = await http.get<{ items: RunAuthorization[] }>(
    `/v1/pm/task-runs/${runId}/execution-authorization`
  );
  return items(payload);
}

/** `GET /v1/pm/task-runs/{run_id}/commands`：Worker 命令台账。 */
export async function listRunCommands(runId: string) {
  const payload = await http.get<{ items: WorkerCommand[] }>(
    `/v1/pm/task-runs/${runId}/commands`
  );
  return items(payload);
}

/** `GET /v1/pm/task-runs/{run_id}/continuation`：Continuation 链。 */
export async function listRunContinuations(runId: string) {
  const payload = await http.get<{ items: RunContinuation[] }>(
    `/v1/pm/task-runs/${runId}/continuation`
  );
  return items(payload);
}

/** `GET /v1/pm/task-runs/{run_id}/commits`：Run 级逐仓交付清单。 */
export async function listRunCommits(runId: string) {
  const payload = await http.get<{ items: RunCommit[] }>(
    `/v1/pm/task-runs/${runId}/commits`
  );
  return items(payload);
}
