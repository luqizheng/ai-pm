import { http } from "@/api/base";

/**
 * 项目总控台聚合读模型（S01 · I5）。
 * 契约：`docs/s01-i5-read-model-contract.md`；事实：`docs/s01-i5-code-facts.md`。
 *
 * 约定：
 * - 六块各自带 `status`：`ok` | `unavailable`——**不可用不得当作 0 / 空集合**；
 * - `as_of` 是该块的新鲜度，不同块不保证同一时刻；
 * - 原始状态一律保留（`source_state`），展示层状态只是附加映射。
 */

export type BlockStatus = "ok" | "unavailable";

export interface CommandCenterBlock<T> {
  status: BlockStatus;
  as_of: string;
  error?: { code: string; message: string };
  data?: T;
}

export interface GoalSummary {
  present: boolean;
  revision_no?: number;
  published_at?: string;
  change_summary?: string;
  pending_proposal: boolean;
}

export interface ProjectSummary {
  project_id: string;
  code: string;
  name: string;
  lifecycle: string;
  archived: boolean;
  customer_name?: string;
  goal: GoalSummary;
  repository_count: number;
  verified_repository_count: number;
  member_count: number;
  last_activity_at?: string;
}

/**
 * S01-01 · G-01 七态展示状态模型 V1.1（用户裁决）：与后端
 * `crates/pm-application/src/command_center.rs` 的 `init_status` 常量同源，
 * 两端必须同批修改。
 *
 * `waiting_for_human` = 系统已产生需要人类处理的事项（球在人手里）——
 * 与 `in_progress`（系统在工作）、`blocked`（外部条件阻塞、当前无可执行的
 * 人类确认动作）三者不得混为一谈。仅作展示层状态，不进入领域实体。
 */
export type InitItemStatus =
  | "not_started"
  | "in_progress"
  | "waiting_for_human"
  | "completed"
  | "blocked"
  | "not_applicable"
  | "undetermined";

export interface InitItem {
  key: string;
  title: string;
  status: InitItemStatus;
  required: boolean;
  source_state?: string;
  detail?: string;
}

export interface InitializationProgress {
  items: InitItem[];
  required_items: string[];
  /** `not_determinable` = 无项目类型/初始化要求配置，不得宣称已完成。 */
  completion: string;
  expanded: boolean;
}

export interface AgentRef {
  principal_id: string;
  display_name: string;
  kind: string;
}

export interface RunRepository {
  repository_id: string;
  base_ref: string;
  base_commit: string;
  workspace_path: string;
}

export interface ActiveRun {
  run_id: string;
  task_id: string;
  task_title: string;
  run_kind: string;
  attempt_no: number;
  status: string;
  stage?: string;
  claimed_at?: string;
  started_at?: string;
  heartbeat_at?: string;
  lease_expires_at?: string;
  agent?: AgentRef;
  repositories: RunRepository[];
}

export interface RunBrief {
  run_id: string;
  task_id: string;
  task_title: string;
  run_kind: string;
  attempt_no: number;
  status: string;
  created_at: string;
  finished_at?: string;
}

export interface DeliverySummary {
  delivered: number;
  partial: number;
  missing: number;
  no_contract: number;
}

export interface WorkSummary {
  task_total: number;
  task_by_status: Record<string, number>;
  /** active 需求总数 / 其中当前修订已 approved（真人定稿）数 */
  requirement_total: number;
  requirement_approved: number;
  active_run_count: number;
  active_runs: ActiveRun[];
  recent_runs: RunBrief[];
  delivery: DeliverySummary;
}

export interface ActionLink {
  kind: string;
  params: Record<string, string>;
}

export interface ActionItem {
  kind: string;
  domain: string;
  id: string;
  project_id: string;
  title: string;
  detail?: string;
  source_state: string;
  updated_at?: string;
  action_hint: string;
  link: ActionLink;
}

export interface HumanActionSummary {
  total: number;
  items: ActionItem[];
}

export interface RiskItem {
  kind: string;
  domain: string;
  source_id: string;
  project_id: string;
  title: string;
  source_state: string;
  severity: "high" | "medium" | "low";
  occurred_at?: string;
  suggestion: string;
  link: ActionLink;
}

export interface RiskSummary {
  total: number;
  items: RiskItem[];
}

export interface ActivityItem {
  id: string;
  activity_type: string;
  source_type?: string;
  source_id?: string;
  occurred_at: string;
  actor: { kind: string; id?: string; name?: string };
  summary: string;
  dedup_key: string;
}

export interface RecentActivity {
  total: number;
  items: ActivityItem[];
}

export interface CommandCenter {
  project_id: string;
  as_of: string;
  blocks: {
    project_summary: CommandCenterBlock<ProjectSummary>;
    initialization_progress: CommandCenterBlock<InitializationProgress>;
    work_summary: CommandCenterBlock<WorkSummary>;
    human_action_summary: CommandCenterBlock<HumanActionSummary>;
    risk_summary: CommandCenterBlock<RiskSummary>;
    recent_activity: CommandCenterBlock<RecentActivity>;
  };
}

/** `GET /v1/pm/projects/{project_id}/command-center`：六块一次拉取；只有活动流分页。 */
export function getCommandCenter(
  projectId: string,
  params: { activity_limit?: number; activity_offset?: number } = {}
) {
  return http.get<CommandCenter>(
    `/v1/pm/projects/${projectId}/command-center`,
    params
  );
}
