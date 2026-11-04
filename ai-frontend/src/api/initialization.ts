/**
 * S01 项目初始化域 API —— 对齐 `docs/s01-backend-api-requirements-v1.md`。
 *
 * 覆盖 S01-02 项目创建、S01-03 仓库管理、S01-04 AI-PM 认知、S01-01 基线确认。
 *
 * 约定（与既有 `src/api/*` 一致）：
 *   - 路径前缀 `/v1/pm/*`（vite 代理剥 `/api`）。
 *   - 列表响应 `{items, total}`，分页加 `page`/`size`。
 *   - 写操作带 `revision` 做乐观锁，冲突由后端返回 409。
 */

import { http } from "@/api/base";
import type { ListResponse, Paged } from "./types";
import type {
  AgentContext,
  BaselineConfirmation,
  BaselineOverview,
  BaselineProposal,
  BaselineRisk,
  CognitionItem,
  CognitionPreparation,
  CognitionRun,
  GitProviderAuthorization,
  InitializationState,
  ProjectBaseline,
  ProjectCodeSuggestion,
  ProjectKnowledge,
  ProjectProfile,
  RepositoryV2,
  RepositoryVerificationOutcome
} from "./types/initialization";

/* ───────────────────────── S01-02 项目创建 ───────────────────────── */

/** S01-02 §3.2：按名称生成 Project Key 建议并检查唯一性。 */
export function suggestProjectCode(name: string) {
  return http.get<ProjectCodeSuggestion>("/v1/pm/projects/suggest-code", {
    name
  });
}

/** S01-02 §5.1：创建请求体。`idempotency_key` 防重复创建。 */
export interface CreateProjectPayload {
  code: string;
  name: string;
  description?: string;
  customer_id?: string;
  owner_principal_id?: string;
  project_type?: string;
  business_domain?: string;
  expected_outcome?: string;
  initial_context?: string;
  idempotency_key?: string;
}

export function createInitializedProject(body: CreateProjectPayload) {
  return http.post<{
    id: string;
    code: string;
    name: string;
    customer_id: string | null;
    owner_principal_id: string;
    status: string;
    revision: number;
    init_status: string;
    created_at: string;
    updated_at: string;
  }>("/v1/pm/projects", body);
}

/** S01-02 §8.2：读取初始化状态聚合。 */
export function getInitializationState(projectId: string) {
  return http.get<InitializationState>(
    `/v1/pm/projects/${projectId}/initialization`
  );
}

/** S01-02 §8.2：更新初始化状态。`completed` 需已有生效基线。 */
export function updateInitializationState(
  projectId: string,
  body: { init_status: string; blocked_reason?: string | null }
) {
  return http.patch<InitializationState>(
    `/v1/pm/projects/${projectId}/initialization`,
    body
  );
}

/** S01-02 §3.4：项目背景信息。 */
export function getProjectProfile(projectId: string) {
  return http.get<ProjectProfile>(`/v1/pm/projects/${projectId}/profile`);
}

export function updateProjectProfile(
  projectId: string,
  body: Partial<
    Pick<
      ProjectProfile,
      | "project_type"
      | "business_domain"
      | "expected_outcome"
      | "initial_context"
    >
  >
) {
  return http.patch<ProjectProfile>(
    `/v1/pm/projects/${projectId}/profile`,
    body
  );
}

/* ───────────────────────── S01-03 仓库管理 ───────────────────────── */

/** S01-03 §4：仓库列表（v2 语义：多仓库 + Primary + Branch/Commit）。 */
export function listRepositoriesV2(projectId: string) {
  return http.get<ListResponse<RepositoryV2>>(
    `/v1/pm/projects/${projectId}/repositories`
  );
}

export interface RepositoryV2Payload {
  name: string;
  url: string;
  provider?: string;
  repo_owner?: string | null;
  role: string;
  default_branch?: string | null;
  selected_branch?: string | null;
  selected_commit?: string | null;
  description?: string | null;
}

export function createRepositoryV2(
  projectId: string,
  body: RepositoryV2Payload
) {
  return http.post<RepositoryV2>(
    `/v1/pm/projects/${projectId}/repositories`,
    body
  );
}

export function updateRepositoryV2(
  repositoryId: string,
  body: RepositoryV2Payload & { revision: number }
) {
  return http.patch<RepositoryV2>(`/v1/pm/repositories/${repositoryId}`, body);
}

/**
 * S01-03 §8.3：逻辑断开。**不删行、不删历史**。
 * 后端若仍以 DELETE 表达断开，前端优先用 disconnect 端点。
 */
export function disconnectRepository(repositoryId: string) {
  return http.post<RepositoryV2>(
    `/v1/pm/repositories/${repositoryId}/disconnect`,
    {}
  );
}

/** S01-03 §8.1：恢复关联，需重新核验权限与版本。 */
export function restoreRepository(repositoryId: string) {
  return http.post<RepositoryV2>(
    `/v1/pm/repositories/${repositoryId}/restore`,
    {}
  );
}

/** S01-03 §3.2：指定 Primary（原 Primary 自动降级）。 */
export function setPrimaryRepository(repositoryId: string) {
  return http.post<RepositoryV2>(
    `/v1/pm/repositories/${repositoryId}/set-primary`,
    {}
  );
}

/**
 * S01-03 §6.2：人类确认后更新项目采用版本。
 * **不自动更新**，也不触碰正式基线。
 */
export function updateRepositoryVersion(
  repositoryId: string,
  body: { selected_branch: string; selected_commit: string; reason?: string }
) {
  return http.post<RepositoryV2>(
    `/v1/pm/repositories/${repositoryId}/update-version`,
    body
  );
}

/** S01-03 §7：核验。返回逐项检查结果与 `head_changed` 提示位。 */
export function verifyRepositoryV2(repositoryId: string) {
  return http.post<RepositoryVerificationOutcome>(
    `/v1/pm/repositories/${repositoryId}/verify`,
    {}
  );
}

/** S01-03 §5.2：Provider 授权（凭证不回显，只读 scopes/status）。 */
export function listGitProviders(projectId: string) {
  return http.get<GitProviderAuthorization[]>(
    `/v1/pm/projects/${projectId}/git-providers`
  );
}

export function startGitProviderAuthorization(
  projectId: string,
  body: { provider: string }
) {
  return http.post<{ authorization_url: string }>(
    `/v1/pm/projects/${projectId}/git-providers`,
    body
  );
}

export function revokeGitProviderAuthorization(
  projectId: string,
  provider: string
) {
  return http.delete<{ revoked: boolean }>(
    `/v1/pm/projects/${projectId}/git-providers/${provider}`
  );
}

/* ───────────────────────── S01-04 AI-PM 认知 ───────────────────────── */

/** S01-04 §4.1：自动准备结果。此阶段**不启动**正式 Run。 */
export function getCognitionPreparation(projectId: string) {
  return http.get<CognitionPreparation>(
    `/v1/pm/projects/${projectId}/cognition/preparation`
  );
}

/** S01-04 §4.3：人工启动正式认知分析。 */
export function startCognitionRun(
  projectId: string,
  body: { scope: string[]; repository_ids?: string[] }
) {
  return http.post<CognitionRun>(
    `/v1/pm/projects/${projectId}/cognition/runs`,
    body
  );
}

export function listCognitionRuns(
  projectId: string,
  params: { page?: number; size?: number } = {}
) {
  return http.get<Paged<CognitionRun>>(
    `/v1/pm/projects/${projectId}/cognition/runs`,
    params
  );
}

export function getCognitionRun(projectId: string, runId: string) {
  return http.get<CognitionRun>(
    `/v1/pm/projects/${projectId}/cognition/runs/${runId}`
  );
}

export function cancelCognitionRun(projectId: string, runId: string) {
  return http.post<CognitionRun>(
    `/v1/pm/projects/${projectId}/cognition/runs/${runId}/cancel`,
    {}
  );
}

/** S01-04 §5.2：认知条目列表，支持按 Run / 状态 / 领域筛选。 */
export function listCognitionItems(
  projectId: string,
  params: {
    run_id?: string;
    status?: string;
    domain?: string;
    page?: number;
    size?: number;
  } = {}
) {
  return http.get<Paged<CognitionItem>>(
    `/v1/pm/projects/${projectId}/cognition/items`,
    params
  );
}

export function getCognitionItem(projectId: string, itemId: string) {
  return http.get<CognitionItem>(
    `/v1/pm/projects/${projectId}/cognition/items/${itemId}`
  );
}

/**
 * S01-04 §6.1：逐项处理。
 *
 * **无批量端点** —— 批量仅是前端交互效率，后端始终逐条落审计。
 */
export function decideCognitionItem(
  projectId: string,
  itemId: string,
  body: {
    action: string;
    content?: string;
    note?: string;
  }
) {
  return http.patch<CognitionItem>(
    `/v1/pm/projects/${projectId}/cognition/items/${itemId}`,
    body
  );
}

/** S01-04 §5.3：正式知识列表。 */
export function listProjectKnowledge(
  projectId: string,
  params: {
    category?: string;
    status?: string;
    page?: number;
    size?: number;
  } = {}
) {
  return http.get<Paged<ProjectKnowledge>>(
    `/v1/pm/projects/${projectId}/knowledge`,
    params
  );
}

export function getProjectKnowledge(projectId: string, knowledgeId: string) {
  return http.get<ProjectKnowledge>(
    `/v1/pm/projects/${projectId}/knowledge/${knowledgeId}`
  );
}

export function getKnowledgeHistory(projectId: string, knowledgeId: string) {
  return http.get<ListResponse<ProjectKnowledge>>(
    `/v1/pm/projects/${projectId}/knowledge/${knowledgeId}/history`
  );
}

/** S01-04 §8.1：设置/取消核心认知。只能由人类确认。 */
export function setCoreKnowledge(
  projectId: string,
  knowledgeId: string,
  body: { is_core: boolean }
) {
  return http.patch<ProjectKnowledge>(
    `/v1/pm/projects/${projectId}/knowledge/${knowledgeId}/core`,
    body
  );
}

/** S01-04 §8.3：Agent Context 检查（只读追溯）。 */
export function getAgentContext(
  projectId: string,
  params: { task_id?: string; repository_id?: string } = {}
) {
  return http.get<AgentContext>(
    `/v1/pm/projects/${projectId}/agent-context`,
    params
  );
}

/* ───────────────────────── S01-01 基线确认 ───────────────────────── */

/** S01-01 §5：基线提案列表。 */
export function listBaselineProposals(
  projectId: string,
  params: { status?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<BaselineProposal>>(
    `/v1/pm/projects/${projectId}/baseline/proposals`,
    params
  );
}

export function getBaselineProposal(projectId: string, proposalId: string) {
  return http.get<BaselineProposal>(
    `/v1/pm/projects/${projectId}/baseline/proposals/${proposalId}`
  );
}

/** S01-01 §3.1：要求修订 → 生成新 Proposal Version。 */
export function reviseBaselineProposal(
  projectId: string,
  proposalId: string,
  body: { instruction: string }
) {
  return http.post<BaselineProposal>(
    `/v1/pm/projects/${projectId}/baseline/proposals/${proposalId}/revise`,
    body
  );
}

/** S01-01 §3.1：拒绝。保留记录，不删除提案与证据。 */
export function rejectBaselineProposal(
  projectId: string,
  proposalId: string,
  body: { reason: string }
) {
  return http.post<BaselineProposal>(
    `/v1/pm/projects/${projectId}/baseline/proposals/${proposalId}/reject`,
    body
  );
}

/** S01-01 §6：生成失败 / 基准过期后重新生成。 */
export function regenerateBaselineProposal(
  projectId: string,
  proposalId: string
) {
  return http.post<BaselineProposal>(
    `/v1/pm/projects/${projectId}/baseline/proposals/${proposalId}/regenerate`,
    {}
  );
}

/** S01-01 §4.3：风险列表。 */
export function listBaselineRisks(projectId: string, proposalId: string) {
  return http.get<ListResponse<BaselineRisk>>(
    `/v1/pm/projects/${projectId}/baseline/proposals/${proposalId}/risks`
  );
}

/**
 * S01-01 §4.3：人类裁决风险。
 *
 * 与 `ai_status` 独立 —— 此处写入的是 `human_decision`。
 */
export function decideBaselineRisk(
  projectId: string,
  proposalId: string,
  riskId: string,
  body: { human_decision: string; note?: string }
) {
  return http.post<BaselineRisk>(
    `/v1/pm/projects/${projectId}/baseline/proposals/${proposalId}/risks/${riskId}/decide`,
    body
  );
}

/**
 * S01-01 §4.4 + §7.2：正式确认。
 *
 * 必须携带并发令牌 `expected_current_baseline_id` 与 `expected_proposal_version`；
 * 冲突返回 409 `baseline_conflict` / `proposal_version_conflict` / `baseline_expired`。
 */
export function confirmBaseline(
  projectId: string,
  proposalId: string,
  body: {
    confirmation_note: string;
    expected_current_baseline_id: string | null;
    expected_proposal_version: number;
    idempotency_key: string;
  }
) {
  return http.post<{
    baseline_id: string;
    baseline_version: number;
    current_baseline_id: string;
    confirmed_by: string;
    confirmed_at: string;
    proposal_id: string;
    proposal_version: number;
    repository_baselines: Array<{
      repository_id: string;
      branch: string | null;
      commit_sha: string | null;
    }>;
    confirmation_note: string | null;
  }>(
    `/v1/pm/projects/${projectId}/baseline/proposals/${proposalId}/confirm`,
    body
  );
}

/** S01-01 §5：当前基线总览 + 历史。 */
export function getBaselineOverview(projectId: string) {
  return http.get<BaselineOverview>(`/v1/pm/projects/${projectId}/baseline`);
}

/** S01-01 §5：历史版本列表。 */
export function listBaselineHistory(projectId: string) {
  return http.get<ListResponse<ProjectBaseline>>(
    `/v1/pm/projects/${projectId}/baseline/history`
  );
}

/** S01-01 §5：版本详情 + 确认记录。 */
export function getBaselineVersion(projectId: string, baselineId: string) {
  return http.get<{
    baseline: ProjectBaseline;
    confirmation: BaselineConfirmation | null;
  }>(`/v1/pm/projects/${projectId}/baseline/history/${baselineId}`);
}
