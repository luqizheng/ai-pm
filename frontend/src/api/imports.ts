import { http } from "@/shell/api";
import type {
  ImportListItem,
  ListResponse,
  ProjectImport,
  ProposedImport
} from "./types";

/** 启动导入：项目需为空且有至少一个 verified 仓库。 */
export function startImport(projectId: string) {
  return http.post<{ import_id: string; status: string }>(
    `/v1/pm/projects/${projectId}/imports`,
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