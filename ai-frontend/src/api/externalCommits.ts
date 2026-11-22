import { http } from "@/api/base";

/**
 * P0 外部 Commit 采纳闭环客户端（迁移 0054）。
 *
 * 闭环：发现（discover 扫描锚点区间）→ AI 分析（analyze，人类触发）→
 * 澄清（clarifications 答复）→ 采纳（decision：approved/no_update/rejected，
 * 人类决定）→ 关闭（close，知识提案全部终态校验）。
 *
 * 硬约束的前端落点：发现 ≠ 采纳（状态徽标分开呈现）；失败可重试不吞错
 * （analyze 失败行 status=failed，UI 给重试入口）；「无需澄清」是显式
 * 记录（clarification_state=not_needed），UI 呈现而非跳过。
 */

/** 列表行（GET /v1/pm/projects/{id}/external-commits） */
export interface ExternalCommitListItem {
  id: string;
  repository_id: string;
  repository_name: string | null;
  commit_sha: string;
  branch: string;
  author_name: string;
  committed_at: string;
  subject: string;
  status: string;
  analysis_mode: string | null;
  clarification_state: string;
  open_questions: number;
  adoption_decision: string | null;
  attempts: number;
  last_error: string | null;
  discovered_at: string;
}

/** 详情行（GET /v1/pm/external-commits/{id}） */
export interface ExternalCommitDetail extends ExternalCommitListItem {
  project_id: string;
  changed_files: Array<{ path: string; change: string }>;
  analysis: CommitAnalysisPayload | null;
  analyzed_at: string | null;
  adoption_note: string | null;
  adopted_by: string | null;
  adopted_at: string | null;
  knowledge_proposals: Array<Record<string, unknown>>;
  adopted_revision_ids: string[];
}

/** 分析产物（AI 产出恒 draft/unverified；采纳须人类批准） */
export interface CommitAnalysisPayload {
  summary: string;
  analysis_mode: string;
  related_objects: Array<{
    domain: string;
    id: string;
    title: string;
    relation: string;
  }>;
  deviation: { detected: boolean; note: string } | null;
  knowledge_updates: Array<{
    doc_key: string;
    doc_type: string;
    title: string;
    summary: string;
    proposed_markdown: string;
  }>;
}

export interface Clarification {
  id: string;
  external_commit_id: string;
  question: string;
  answer: string | null;
  answered_by: string | null;
  answered_at: string | null;
  status: string;
  created_at: string;
}

export interface RepositoryScanOutcome {
  repository_id: string;
  repository_name: string;
  branch: string;
  remote_head: string | null;
  anchor_used: string | null;
  discovered: number;
  truncated: boolean;
  note: string | null;
  error: string | null;
}

export interface DiscoverOutcome {
  scanned_repositories: number;
  discovered_commits: number;
  repositories: RepositoryScanOutcome[];
}

export async function discoverExternalCommits(
  projectId: string,
  repositoryId?: string
): Promise<DiscoverOutcome> {
  return await http.post<DiscoverOutcome>(
    `/v1/pm/projects/${projectId}/external-commits/discover`,
    repositoryId ? { repository_id: repositoryId } : {}
  );
}

export async function listExternalCommits(
  projectId: string,
  status?: string
): Promise<ExternalCommitListItem[]> {
  const params = new URLSearchParams({ project_id: projectId });
  if (status) params.set("status", status);
  const res = await http.get<{ items: ExternalCommitListItem[]; total: number }>(
    `/v1/pm/projects/${projectId}/external-commits?${params.toString()}`
  );
  return res.items ?? [];
}

export async function getExternalCommit(
  commitId: string
): Promise<{ commit: ExternalCommitDetail; clarifications: Clarification[]; repository_name: string }> {
  return await http.get(`/v1/pm/external-commits/${commitId}`);
}

export interface AnalyzeOutcome {
  id: string;
  status: string;
  analysis_mode: string | null;
  clarification_state: string | null;
  error: string | null;
}

export async function analyzeExternalCommit(commitId: string): Promise<AnalyzeOutcome> {
  return await http.post(`/v1/pm/external-commits/${commitId}/analyze`, {});
}

export async function answerClarification(
  commitId: string,
  clarificationId: string,
  answer: string
): Promise<Clarification> {
  return await http.post(
    `/v1/pm/external-commits/${commitId}/clarifications/${clarificationId}/answer`,
    { answer }
  );
}

export interface AdoptionOutcome {
  id: string;
  status: string;
  adoption_decision: string;
  knowledge_proposals: Array<Record<string, unknown>>;
}

export async function decideAdoption(
  commitId: string,
  decision: "approved" | "no_update" | "rejected",
  note?: string
): Promise<AdoptionOutcome> {
  return await http.post(`/v1/pm/external-commits/${commitId}/decision`, {
    decision,
    note: note ?? null
  });
}

export async function closeAdoption(commitId: string): Promise<AdoptionOutcome> {
  return await http.post(`/v1/pm/external-commits/${commitId}/close`, {});
}

export async function getCommitDiff(
  commitId: string
): Promise<{ diff: string; truncated: boolean; commit_sha: string }> {
  return await http.get(`/v1/pm/external-commits/${commitId}/diff`);
}
