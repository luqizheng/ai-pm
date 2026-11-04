import { http } from "@/api/base";
import type { ListResponse } from "./types";

/**
 * 验收证据读模型 + 验收动作（T-070 §5.3 / §5.4）。
 *
 * 三个读端点是任务级只读投影；写路径**零新增**——直接调 T-043 既有的
 * `POST /v1/pm/review-approvals/{approval_id}/decision`（单事务、锚定回读、
 * E2 门复核、审计 + 事件）。V1 只有 `approved` / `rejected`，不做部分通过（Q3）。
 */

/** 一条 review finding（blocker 置顶由后端排序保证）。 */
export interface ReviewFinding {
  id: string;
  review_run_id: string;
  review_attempt_no: number;
  review_run_status: string;
  criterion_id: string | null;
  kind: string;
  severity: "blocker" | "warning" | "info" | string;
  code_location: string | null;
  message: string;
  evidence_ref: string | null;
  created_at: string;
}

export function listReviewFindings(taskId: string) {
  return http.get<ListResponse<ReviewFinding>>(
    `/v1/pm/tasks/${taskId}/review-findings`
  );
}

/**
 * 验收标准逐条裁决（当前快照）：`evaluated=false` 表示从未被任何成功
 * review run 覆盖——verdict 为 null，不造 `awaiting_manual` 假值。
 */
export interface CriterionVerdict {
  criterion_id: string;
  ordinal: number;
  text: string;
  verification_kind: string;
  evaluated: boolean;
  review_run_id: string | null;
  review_attempt_no: number | null;
  verdict: string | null;
  evidence: Record<string, unknown> | null;
  decided_by: string | null;
  decided_at: string | null;
  verdict_created_at: string | null;
}

export function listCriterionVerdicts(taskId: string) {
  return http.get<ListResponse<CriterionVerdict>>(
    `/v1/pm/tasks/${taskId}/criterion-verdicts`
  );
}

/** 审批历史（含 pending），与 `/v1/pm/review-approvals` 同一读模型的 task 级投影。 */
export interface ReviewApproval {
  id: string;
  task_id: string;
  review_run_id: string;
  delivery_run_id: string;
  kind: string;
  status: "pending" | "approved" | "rejected" | string;
  decided_by: string | null;
  decision_reason: string | null;
  decided_at: string | null;
  created_at: string;
  task_title: string;
}

export function listTaskReviewApprovals(taskId: string) {
  return http.get<ListResponse<ReviewApproval>>(
    `/v1/pm/tasks/${taskId}/review-approvals`
  );
}

/** D5 裁决回执（`ApprovalOutcome`，`#[serde(tag = "outcome")]`）。 */
export type ReviewDecisionOutcome =
  | { outcome: "completed"; task_id: string; proposal_id: string | null }
  | { outcome: "changes_requested"; task_id: string }
  | { outcome: "already_decided"; status: string };

/**
 * 验收动作：复用 D5 decision 端点（Q3）。`anchor_drift` /
 * `completion_gate_refused` 等 409 由 http 层原样抛错，调用方透出提示、
 * 不静默重试。
 */
export function decideReviewApproval(
  approvalId: string,
  body: {
    project_id: string;
    decision: "approved" | "rejected";
    reason?: string;
  }
) {
  return http.post<ReviewDecisionOutcome>(
    `/v1/pm/review-approvals/${approvalId}/decision`,
    body
  );
}
