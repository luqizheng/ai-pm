import { http } from "@/api/base";
import type {
  ListResponse,
  Repository,
  RepositoryRole,
  RepositoryStatus,
  SshKey
} from "./types";

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

/** 列出项目全部密钥（含已吊销；默认把排最前）。 */
export function listSshKeys(projectId: string) {
  return http.get<ListResponse<SshKey>>(
    `/v1/pm/projects/${projectId}/ssh-keys`
  );
}

/** 幂等：已有 active 默认把返回现有；`rotate=true` 才重新生成默认把。 */
export function createSshKey(projectId: string, rotate = false) {
  return http.post<SshKey>(`/v1/pm/projects/${projectId}/ssh-keys`, {
    rotate
  });
}

export interface VerifyOutcome {
  repository_id: string;
  status: RepositoryStatus;
  default_branch: string | null;
  head_commit_sha: string | null;
  error_code: string | null;
  error_detail: string | null;
}

/** `git ls-remote` 校验可达性并回写 verified/unreachable。 */
export function verifyRepository(id: string) {
  return http.post<VerifyOutcome>(`/v1/pm/repositories/${id}/verify`, {});
}

/** 归档（保留行与历史，只把 status 置 archived）。 */
export function archiveRepository(id: string) {
  return http.post<Repository>(`/v1/pm/repositories/${id}/archive`, {});
}

/** 吊销指定 key；仍被 verified 仓库绑定引用时返回 409 key_in_use。 */
export function revokeSshKey(keyId: string) {
  return http.delete<{ id: string }>(`/v1/pm/ssh-keys/${keyId}`);
}

/**
 * 粘贴导入外部私钥（迁移 0022 多把模型）。
 * `label` 即身份：同名 = 替换该把，新名 = 新增一把；项目第一把自动成为默认。
 * 后端经 ssh-keygen 校验并推导公钥/指纹；私钥信封加密落库、不再回显。
 * 无效私钥（格式错误 / 带口令）→ 400，错误体 {message}。
 */
export function importSshKey(
  projectId: string,
  privateKey: string,
  label: string
) {
  return http.post<SshKey>(`/v1/pm/projects/${projectId}/ssh-keys/import`, {
    private_key: privateKey,
    label
  });
}

/** 仓库绑定/解绑一把 key（null = 跟随项目默认）。 */
export function bindRepositoryKey(
  repositoryId: string,
  sshKeyId: string | null
) {
  return http.put<Repository>(`/v1/pm/repositories/${repositoryId}/ssh-key`, {
    ssh_key_id: sshKeyId
  });
}
