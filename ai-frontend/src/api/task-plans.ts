import { http } from "@/api/base";

/**
 * S04 任务计划与执行规划读模型（pm-api `pm_plans.rs` / `pm_tasks.rs`）。
 *
 * 领域语义（docs/scenarios/s04/ui + S04 设计提案）：
 * - **Task Plan 是计划聚合层**，正式任务全部复用 `delivery.task`（13 态状态机
 *   / 依赖 / 仓库绑定 / 验收标准原样使用）——不新建第二套任务体系。
 * - **计划四态**：draft / in_review / approved / superseded；只有 approved
 *   计划能正式化（materialize）成正式任务（后端硬校验）。
 * - **计划修订五态**：draft / submitted / approved / rejected / superseded；
 *   提交后内容冻结（与 S03 设计修订同款纪律）。
 * - **依赖方向**：上游 = 本任务依赖的（前置），下游 = 依赖本任务的（后继）。
 * - **排期**：planned_start_at / planned_end_at / estimate_hours 是**生效排期**
 *   （人工可调），不是计划建议；未排期任务归入「未排期托盘」。
 * - **执行准备度三态**：ready / needs_attention / blocked；六段检查 A–F
 *   （任务定义 / 依据追溯 / 依赖校验 / 工程准备 / 验收交付 / 人工批准）。
 */

/** 计划四态。 */
export type TaskPlanStatus = "draft" | "in_review" | "approved" | "superseded";

/** 计划修订五态。 */
export type TaskPlanRevisionState =
  | "draft"
  | "submitted"
  | "approved"
  | "rejected"
  | "superseded";

/** 计划聚合根。 */
export interface TaskPlanEntry {
  id: string;
  project_id: string;
  code: string;
  title: string;
  status: TaskPlanStatus;
  current_revision_id: string | null;
  /** 钉定的 S03 实施基线（Q4）。 */
  source_baseline_id: string | null;
  row_revision: number;
  created_at: string;
  updated_at: string;
}

/** 计划修订（不可变内容版本）。 */
export interface TaskPlanRevision {
  id: string;
  plan_id: string;
  version: number;
  content: Record<string, unknown>;
  state: TaskPlanRevisionState;
  content_hash: string;
  source: string;
  instructions: string | null;
  submitted_at: string | null;
  approved_at: string | null;
  created_at: string;
}

/** 计划内任务草案（content.tasks[] 元素）。 */
export interface PlanTaskDraft {
  key: string;
  title: string;
  description?: string | null;
  acceptance_criteria?: string[];
  depends_on?: string[];
  repository_ids?: string[];
  priority?: string;
  change?: string;
}

/** 已正式化任务（plan_revision_id 锚定）。 */
export interface MaterializedTask {
  task_id: string;
  plan_task_key: string | null;
  title: string;
  status: string;
}

/** 计划详情。 */
export interface TaskPlanDetail {
  plan: TaskPlanEntry;
  revisions: TaskPlanRevision[];
  approvals: Array<Record<string, unknown>>;
  materialized_tasks: MaterializedTask[];
}

/** 计划进度与阻塞清单。 */
export interface TaskPlanProgress {
  plan_id: string;
  status: string;
  revision_id: string | null;
  total_tasks: number;
  by_status: Record<string, number>;
  blocked: Array<Record<string, unknown>>;
  planned_tasks: number;
}

// ─────────────────────────────────────────────────────── 单任务详情（S04-UI-03）

/** 依据追溯行。 */
export interface TaskTrace {
  kind: "requirement" | "design" | "adr" | "baseline" | string;
  target_id: string;
  code: string | null;
  title: string | null;
  version: string | null;
  state: string | null;
}

/** 依赖行（上游 / 下游）。 */
export interface TaskDependency {
  id: string;
  title: string;
  status: string;
  priority: string;
  direction: "upstream" | "downstream" | string;
}

/** 仓库绑定行。 */
export interface TaskRepositoryBinding {
  repository_id: string;
  repository_name: string;
  base_ref: string | null;
  base_commit: string | null;
  write_access: boolean;
  path_scope: string[];
  role: string | null;
  repository_verified: boolean;
}

/** 验收标准行。 */
export interface TaskAcceptanceCriterion {
  id: string;
  ordinal: number;
  text: string;
  verification_kind: string;
}

/** 人工审批行。 */
export interface TaskApproval {
  id: string;
  kind: string;
  status: string;
  required_role: string | null;
  decided_by: string | null;
  decision_reason: string | null;
  decided_at: string | null;
  created_at: string;
}

/** 审计历史行。 */
export interface TaskHistoryEntry {
  id: string;
  actor_id: string;
  actor_name: string | null;
  action: string;
  created_at: string;
}

/** 实施基线快照。 */
export interface TaskBaselineSnapshot {
  id: string;
  code: string;
  status: string;
  content: Record<string, unknown>;
}

/** 单任务详情。 */
export interface TaskDetailView {
  task: {
    id: string;
    project_id: string;
    title: string;
    description: string | null;
    status: string;
    priority: string;
    assignee_id: string | null;
    revision: number;
    goal: string | null;
    constraints: Record<string, unknown>;
    deliverables: unknown[];
    git_branch: string | null;
    planned_start_at: string | null;
    planned_end_at: string | null;
    estimate_hours: number | null;
    implementation_baseline_id: string | null;
    plan_revision_id: string | null;
    created_at: string;
    updated_at: string;
  };
  traces: TaskTrace[];
  dependencies: TaskDependency[];
  repositories: TaskRepositoryBinding[];
  criteria: TaskAcceptanceCriterion[];
  approvals: TaskApproval[];
  history: TaskHistoryEntry[];
  baseline: TaskBaselineSnapshot | null;
}

// ─────────────────────────────────────────────────────── 依赖图（S04-UI-05）

/** 依赖图节点。 */
export interface DependencyGraphNode {
  id: string;
  title: string;
  status: string;
  priority: string;
  planned_start_at: string | null;
  planned_end_at: string | null;
  estimate_hours: number | null;
  repository_name: string | null;
}

/** 依赖图边。 */
export interface DependencyGraphEdge {
  from_task_id: string;
  to_task_id: string;
  dependency_type: string;
}

/** 项目级依赖图。 */
export interface DependencyGraph {
  nodes: DependencyGraphNode[];
  edges: DependencyGraphEdge[];
  /** 节点分层（最长路径分层；根层 = 0）。 */
  layers: string[][];
  /** 关键路径节点。 */
  critical_path: string[];
  /** 并行组（同层且互相无依赖边）。 */
  parallel_groups: string[][];
  /** 环依赖（成环即阻断执行规划）。 */
  cycles: string[][];
}

// ───────────────────────────────────────────── 执行准备度（S04-UI-07）

/** 准备度检查项。 */
export interface ReadinessCheck {
  section: string;
  key: string;
  passed: boolean;
  issues: string[];
}

/** 准备度三态汇总。 */
export interface ReadinessSummary {
  status: "ready" | "needs_attention" | "blocked" | string;
  ready_count: number;
  total_count: number;
  blocked_count: number;
}

/** 单任务准备度。 */
export interface TaskReadiness {
  task_id: string;
  title: string;
  status: string;
  summary: ReadinessSummary;
  checks: ReadinessCheck[];
}

/** 项目级执行准备度。 */
export interface ExecutionReadiness {
  summary: ReadinessSummary;
  tasks: TaskReadiness[];
}

function items<T>(payload: { items?: T[] } | null | undefined): T[] {
  return payload?.items ?? [];
}

/** `GET /v1/pm/projects/{project_id}/task-plans`：计划列表。 */
export async function listTaskPlans(projectId: string) {
  const payload = await http.get<{ items: TaskPlanEntry[] }>(
    `/v1/pm/projects/${projectId}/task-plans`
  );
  return items(payload);
}

/** `GET /v1/pm/task-plans/{plan_id}`：计划详情（修订链 + 已正式化任务）。 */
export function getTaskPlan(planId: string) {
  return http.get<TaskPlanDetail>(`/v1/pm/task-plans/${planId}`);
}

/** `GET /v1/pm/task-plans/{plan_id}/progress`：计划进度与阻塞清单。 */
export function getTaskPlanProgress(planId: string) {
  return http.get<TaskPlanProgress>(`/v1/pm/task-plans/${planId}/progress`);
}

/** `GET /v1/pm/tasks/{task_id}`：单任务完整读模型。 */
export function getTaskDetail(taskId: string) {
  return http.get<TaskDetailView>(`/v1/pm/tasks/${taskId}`);
}

/** `GET /v1/pm/projects/{project_id}/dependency-graph`：项目级依赖图。 */
export function getDependencyGraph(projectId: string) {
  return http.get<DependencyGraph>(
    `/v1/pm/projects/${projectId}/dependency-graph`
  );
}

/** `GET /v1/pm/projects/{project_id}/execution-readiness`：执行准备度。 */
export function getExecutionReadiness(projectId: string) {
  return http.get<ExecutionReadiness>(
    `/v1/pm/projects/${projectId}/execution-readiness`
  );
}

/**
 * `PATCH /v1/pm/tasks/{task_id}/schedule`：写生效排期。
 *
 * 三值都是「全量替换」语义（null = 取消该项），不写部分更新——排期是
 * 单一整体（起止 + 工时），避免半更新产出非法区间。
 */
export function setTaskSchedule(
  taskId: string,
  body: {
    planned_start_at: string | null;
    planned_end_at: string | null;
    estimate_hours: number | null;
  }
) {
  return http.patch<{ task: TaskDetailView["task"] }>(
    `/v1/pm/tasks/${taskId}/schedule`,
    body
  );
}

/** 计划修订正文里的任务草案列表（content.tasks[]）。 */
export function planTaskDrafts(
  revision: TaskPlanRevision | null
): PlanTaskDraft[] {
  const raw = revision?.content?.tasks;
  if (!Array.isArray(raw)) return [];
  return raw.filter((item): item is PlanTaskDraft =>
    Boolean(
      item && typeof item === "object" && "key" in item && "title" in item
    )
  );
}
