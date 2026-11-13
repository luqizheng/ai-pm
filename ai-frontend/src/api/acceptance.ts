import { http } from "@/api/base";

/**
 * 交付验收工作台读模型（T-070 / `docs/tasks/ai-pm-delivery-acceptance-workbench-v1.md` §5.1）。
 *
 * 四概念分离：任务生命周期 / 执行 Run / 仓库交付事实 / 人工验收决定——
 * 徽标（badge）与动作区状态（review_status）由后端统一计算，前端**禁止**
 * 自行拼表推导，只消费字段渲染（Q2）。
 */

export type AcceptanceBadge = "eligible" | "blocked" | "missing";
export type AcceptanceReviewStatus =
  | "none"
  | "running"
  | "awaiting_decision"
  | "decided";

/** 交付摘要（任务级，来自交付清单视图聚合）。 */
export interface DeliverySummary {
  expected_repos: number;
  delivered_repos: number;
  /** delivery_required 却没有 commit 的仓名。 */
  missing_repos: string[];
  /** 最新 delivery run 集成事实（passed / failed / not_run）；无 run 时为 null。 */
  integration_status: string | null;
}

/** 待验收总表的一行。 */
export interface AcceptanceTask {
  task_id: string;
  title: string;
  status: string;
  badge: AcceptanceBadge;
  delivery_summary: DeliverySummary;
  blocker_count: number;
  latest_delivery_run_id: string | null;
  latest_attempt_no: number | null;
  review_status: AcceptanceReviewStatus;
  /** 可操作时为 pending 审批行 id，否则为 null。 */
  pending_approval_id: string | null;
  created_at: string;
}

/** 顶部三指标：三者维度不同，不得混同。 */
export interface AcceptanceCounts {
  pending: number;
  eligible: number;
  blocked: number;
}

export interface AcceptanceOverview {
  project_id: string;
  counts: AcceptanceCounts;
  /** 已按处理优先级排序：blocked → missing → eligible，组内 created_at DESC。 */
  tasks: AcceptanceTask[];
  total: number;
}

/** `GET /v1/pm/acceptance-overview?project_id=`。 */
export function getAcceptanceOverview(projectId: string) {
  return http.get<AcceptanceOverview>("/v1/pm/acceptance-overview", {
    project_id: projectId
  });
}

/** 多仓交付清单行（`delivery.v_task_delivery_manifest` 投影，D4 读模型零改动）。 */
export interface TaskDeliveryRow {
  task_id: string;
  project_id: string;
  repository_id: string;
  repository_name: string;
  write_access: boolean;
  delivery_required: boolean;
  latest_run_id: string | null;
  latest_attempt_no: number | null;
  integration_status: string | null;
  latest_submitted_run_id: string | null;
  base_commit: string | null;
  commit_sha: string | null;
  changed_files: number | null;
  test_evidence: {
    submitted_at?: string;
    tests?: { name: string; passed: boolean }[];
  } & Record<string, unknown>;
  branch: string | null;
  commit_author: string | null;
  commit_authored_at: string | null;
  expected_delivery_count: number;
  actual_delivery_count: number;
  /** 任务级三值：delivered / partial / never_submitted（≠ 已验收，D4 §2.3）。 */
  delivery_state: string;
}

/** 多仓交付清单：复用 D4 只读端点（零改动）。 */
export function listTaskDelivery(taskId: string) {
  return http.get<{
    task_id: string;
    project_id: string;
    items: TaskDeliveryRow[];
    total: number;
  }>(`/v1/pm/tasks/${taskId}/delivery`);
}
