import { http } from "@/shell/api";
import type { ListResponse, Repository, RepositoryRole, SshKey } from "./types";

/**
 * 项目 ↔ 代码仓库关系（pm-api `/v1/pm/*`，P0 纯 CRUD）。
 *
 * 约定：列表返回裸 payload `{items,total}`；错误体 `{message}`（@digua/http 原样透传）。
 */

export interface RepositoryPayload {
  name: string;
  url: string;
  role: RepositoryRole;
  default_branch?: string | null;
  description?: string | null;
}

export function listRepositories(projectId: string) {
  return http.get<ListResponse<Repository>>(
    `/v1/pm/projects/${projectId}/repositories`
  );
}

export function createRepository(projectId: string, body: RepositoryPayload) {
  return http.post<Repository>(
    `/v1/pm/projects/${projectId}/repositories`,
    body
  );
}

export function updateRepository(
  id: string,
  body: RepositoryPayload & { revision: number }
) {
  return http.patch<Repository>(`/v1/pm/repositories/${id}`, body);
}

export function deleteRepository(id: string) {
  return http.delete<{ id: string }>(`/v1/pm/repositories/${id}`);
}

/** 未生成密钥时后端 404；调用方按「无密钥」处理。 */
export function getSshKey(projectId: string) {
  return http.get<SshKey>(`/v1/pm/projects/${projectId}/ssh-key`);
}

/** 生成或重新生成（一项目一把，重复调用即替换）。 */
export function generateSshKey(projectId: string) {
  return http.post<SshKey>(`/v1/pm/projects/${projectId}/ssh-key`, {});
}

export function deleteSshKey(projectId: string) {
  return http.delete<{ project_id: string }>(
    `/v1/pm/projects/${projectId}/ssh-key`
  );
}
