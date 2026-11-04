import { http } from "@/api/base";
import type {
  ImportListItem,
  ListResponse,
  ProjectImport,
  ProposedImport
} from "./types";

/**
 * 启动导入：项目需为空，且 `repositoryIds` **必填且非空**（I6-B N-3）。
 *
 * 导入集合由调用方明确指定，服务端在**创建时固化**；重复提交同一
 * `idempotencyKey` 会命中同一次导入（服务端幂等重放）。
 */
export function startImport(
  projectId: string,
  repositoryIds: string[],
  idempotencyKey?: string
) {
  return http.post<{ import_id: string; status: string; replayed?: boolean }>(
    `/v1/pm/projects/${projectId}/imports`,
    {
      repository_ids: repositoryIds,
      idempotency_key: idempotencyKey ?? null
    }
  );
}

/** 显式终止 `proposed` 态的导入（C-2：提案挂起时也能释放名额）。 */
export function terminateImport(projectId: string, importId: string) {
  return http.post<{ status: string; terminated: boolean }>(
    `/v1/pm/projects/${projectId}/imports/${importId}/terminate`,
    {}
  );
}

export function listImports(projectId: string) {
  return http.get<ListResponse<ImportListItem>>(
    `/v1/pm/projects/${projectId}/imports`
  );
}

export function getImport(projectId: string, importId: string) {
  return http.get<ProjectImport>(
    `/v1/pm/projects/${projectId}/imports/${importId}`
  );
}

export function cancelImport(projectId: string, importId: string) {
  return http.post<{ status: string }>(
    `/v1/pm/projects/${projectId}/imports/${importId}/cancel`,
    {}
  );
}

export interface ApplyImportPayload {
  draft_revision: number;
  project: { code: string; name: string; description?: string };
  milestone_keys: string[];
  requirement_keys: string[];
  task_keys: string[];
  write_knowledge_documents: boolean;
}

/** apply 只生成提案；写入由审批链执行。 */
export function applyImport(
  projectId: string,
  importId: string,
  payload: ApplyImportPayload
) {
  return http.post<ProposedImport>(
    `/v1/pm/projects/${projectId}/imports/${importId}/apply`,
    payload
  );
}
/** 已生成提案的批次（含 approval_id，供审批页整批批准）。 */
export function getImportProposals(projectId: string, importId: string) {
  return http.get<ProposedImport>(
    `/v1/pm/projects/${projectId}/imports/${importId}/proposals`
  );
}
