import { http } from "@/shell/api";
import type {
  ApprovalRequest,
  AuditLog,
  ListResponse,
  Paged,
  Principal,
  Project,
  Requirement,
  Task
} from "./types";

/**
 * AI-PM 业务端点（pm-api 契约）。
 *
 * 约定：与壳的 iam 端点不同，业务端点返回**裸 payload**（无 {code,message,data} 信封），
 * 列表统一 { items, total }；pm-api 实现时按此交付。
 */

export function listProjects(params: { page?: number; size?: number } = {}) {
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

export function listApprovals(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<ApprovalRequest>>("/v1/pm/approvals", params);
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
