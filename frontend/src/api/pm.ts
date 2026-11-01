import { http } from "@/shell/api";
import type {
  ApprovalRequest,
  AuditLog,
  ListResponse,
  Paged,
  Principal,
  Project,
  Requirement,
  RequirementRevision,
  Task
} from "./types";
import type { DecisionRecord, Milestone } from "./workspace-view";

/**
 * AI-PM 业务端点（pm-api 契约）。
 *
 * 约定：与壳的 iam 端点不同，业务端点返回**裸 payload**（无 {code,message,data} 信封），
 * 列表统一 { items, total }；pm-api 实现时按此交付。
 */

export function listProjects(
  params: { customer_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<Project>>("/v1/pm/projects", params);
}

export function listPrincipals(params: { page?: number; size?: number } = {}) {
  return http.get<Paged<Principal>>("/v1/pm/principals", params);
}

export function listRequirements(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<Requirement>>("/v1/pm/requirements", params);
}

export function listTasks(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<Task>>("/v1/pm/tasks", params);
}

export function listMilestones(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<Milestone>>("/v1/pm/milestones", params);
}

export function listApprovals(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<ApprovalRequest>>("/v1/pm/approvals", params);
}

/** 决策中心读模型（审批请求 + 关联变更提案），created_at DESC。 */
export function listDecisions(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<DecisionRecord>>("/v1/pm/decisions", params);
}

export function listAuditLogs(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<ListResponse<AuditLog>>("/v1/pm/audit-logs", params);
}

/**
 * 审批决策（红线确认）。
 * 契约见 mock/fixtures.ts：成功返回 `{id, status, decided_at}`；
 * 404 未知、409 已决策、400 缺 reason 或非法 decision（HttpError.message 已翻译）。
 */
export function decideApproval(
  id: string,
  body: { decision: "approved" | "rejected"; reason: string }
) {
  return http.post<{ id: string; status: string; decided_at: string }>(
    `/v1/pm/approvals/${id}/decide`,
    body
  );
}

/** 修订历史（version 倒序）。契约见 mock/fixtures.ts MockRevision 段。 */
export function listRevisions(requirementId: string) {
  return http.get<ListResponse<RequirementRevision>>(
    `/v1/pm/requirements/${requirementId}/revisions`
  );
}

/** 新建草稿修订：version 自增、content_hash = sha256、created_by = 当前会话 */
export function createRevision(requirementId: string, markdown: string) {
  return http.post<RequirementRevision>(
    `/v1/pm/requirements/${requirementId}/revisions`,
    { content: { markdown } }
  );
}

/** 定稿：draft → approved，同需求其余 approved 转 superseded；非 draft 409 */
export function approveRevision(requirementId: string, revisionId: string) {
  return http.post<RequirementRevision>(
    `/v1/pm/requirements/${requirementId}/revisions/${revisionId}/approve`
  );
}

/**
 * 创建任务（status 固定 backlog，revision=1）。契约见 mock/fixtures.ts 任务段。
 * milestone_id / assignee_id 可空；project 不存在 404。
 */
export function createTask(body: {
  project_id: string;
  milestone_id?: string | null;
  title: string;
  description?: string | null;
  priority?: string;
  assignee_id?: string | null;
}) {
  return http.post<Task>("/v1/pm/tasks", body);
}

/**
 * 编辑任务（全字段 + revision 乐观锁，不匹配 409；title/status/priority 必填）。
 * 返回更新后的完整任务（revision 已 +1）。
 */
export function updateTask(
  id: string,
  body: {
    revision: number;
    title: string;
    description?: string | null;
    status: string;
    priority: string;
    assignee_id?: string | null;
  }
) {
  return http.patch<Task>(`/v1/pm/tasks/${id}`, body);
}
