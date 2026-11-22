import { http } from "@/api/base";
import type { Paged, Project } from "./types";

/** `POST /v1/pm/projects`：建项目并自动落 5 人编制；传 customer_id 挂到已有客户上。 */
export function createProject(body: {
  code: string;
  name: string;
  description?: string;
  customer_id?: string;
}) {
  return http.post<Project>("/v1/pm/projects", body);
}

export function listProjects(
  params: { customer_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<Project>>("/v1/pm/projects", params);
}

/** `GET /v1/pm/projects/{id}`：项目详情（P1 Settings General）。 */
export interface ProjectInfo {
  id: string;
  customer_id: string;
  customer_name: string | null;
  code: string;
  name: string;
  description: string | null;
  status: string;
  revision: number;
  created_at: string;
  updated_at: string;
}

export function getProjectInfo(projectId: string) {
  return http.get<ProjectInfo>(`/v1/pm/projects/${projectId}`);
}

/** `PATCH /v1/pm/projects/{id}`：基本信息（名称/描述）；守卫=生命周期管理者。 */
export function updateProjectInfo(
  projectId: string,
  body: { name: string; description?: string | null }
) {
  return http.patch<ProjectInfo>(`/v1/pm/projects/${projectId}`, body);
}

/** `POST /v1/pm/projects/{id}/close`：关闭项目（危险操作；Product Owner）。 */
export function closeProject(projectId: string, reason?: string) {
  return http.post<{ project_id: string; status: string }>(
    `/v1/pm/projects/${projectId}/close`,
    { reason: reason ?? "project_closed" }
  );
}

/** `POST /v1/pm/projects/{id}/reopen`：重开已关闭项目（同一守卫）。 */
export function reopenProject(projectId: string) {
  return http.post<{ project_id: string; status: string }>(
    `/v1/pm/projects/${projectId}/reopen`,
    {}
  );
}

/** 项目成员行（GET /v1/pm/projects/{id}/members）。role=core.project_member 原值。 */
export interface ProjectMember {
  principal_id: string;
  display_name: string;
  role: string;
  kind: string;
  is_active: boolean;
}

export function listProjectMembers(projectId: string) {
  return http.get<{ items: ProjectMember[] }>(
    `/v1/pm/projects/${projectId}/members`
  );
}

/** 授予项目成员（角色固定 member；owner 只能走所有权转移）。 */
export function grantProjectMember(projectId: string, principalId: string) {
  return http.post<{ principal_id: string; role: string | null }>(
    `/v1/pm/projects/${projectId}/members`,
    { principal_id: principalId }
  );
}

export function revokeProjectMember(projectId: string, principalId: string) {
  return http.delete<{ principal_id: string; role: null }>(
    `/v1/pm/projects/${projectId}/members/${principalId}`
  );
}

export function transferProjectOwnership(
  projectId: string,
  newOwnerPrincipalId: string
) {
  return http.post<{ principal_id: string; role: string }>(
    `/v1/pm/projects/${projectId}/owner-transfer`,
    { new_owner_principal_id: newOwnerPrincipalId }
  );
}

/** 主体候选（GET /v1/pm/principals，邀请下拉用）。 */
export interface PrincipalOption {
  id: string;
  display_name: string;
  kind: string;
  is_active: boolean;
}

export function listPrincipals(params: { is_active?: boolean } = {}) {
  return http.get<Paged<PrincipalOption>>("/v1/pm/principals", params);
}
