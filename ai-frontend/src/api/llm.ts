import { http } from "@/api/base";
import type { ListResponse } from "./types";

/**
 * Team LLM Provider（0041：`ai.team_llm_provider`，Team-owned Resource）。
 * 与旧全局 `/v1/pm/llm/providers*` 不同：实例可多个同类型、带生命周期状态、
 * 乐观锁 revision；API Key 明文不回传——只有 `has_credential` 与末 4 位提示。
 */
export type LlmProviderType =
  | "openai"
  | "gemini"
  | "deepseek"
  | "glm"
  | "minimax"
  | "ollama"
  | "openai_compatible";

export type LlmProviderStatus =
  | "not_configured"
  | "configured"
  | "connected"
  | "failed"
  | "enabled"
  | "disabled"
  | "archived";

export interface LlmProvider {
  id: string;
  name: string;
  provider_type: LlmProviderType;
  endpoint: string;
  /** 调用参数（Phase 3 模型目录前：model 等存这里）。 */
  configuration: { model?: string } & Record<string, unknown>;
  status: LlmProviderStatus;
  timeout_secs: number;
  /** 乐观锁：写请求必须携带 expected_revision = 当前值。 */
  revision: number;
  has_credential: boolean;
  credential_hint: string | null;
  last_tested_at: string | null;
  created_by: string;
  created_at: string;
  updated_at: string;
}

/** 当前主体所属租户（Team ≡ tenant）：Provider 管理面的作用域。 */
export async function resolveTenantId(): Promise<string> {
  const me = await http.get<{
    tenant_id: string;
    memberships?: { team_id: string }[];
  }>("/v1/pm/me");
  // P4-ORG Phase 4b：LLM 面路由锚 = core.team.id（/me memberships[0]）。
  return me.memberships?.[0]?.team_id ?? me.tenant_id;
}

export function listLlmProviders(teamId: string) {
  return http.get<ListResponse<LlmProvider>>(
    `/v1/pm/teams/${teamId}/llm-providers`
  );
}

export interface CreateLlmProviderBody {
  name: string;
  provider_type: LlmProviderType;
  endpoint: string;
  configuration?: { model?: string } & Record<string, unknown>;
  /** 缺省/空 = 只建实例不配凭证（not_configured）。 */
  api_key?: string;
  timeout_secs?: number;
}

export function createLlmProvider(teamId: string, body: CreateLlmProviderBody) {
  return http.post<LlmProvider>(`/v1/pm/teams/${teamId}/llm-providers`, body);
}

export interface UpdateLlmProviderBody {
  expected_revision: number;
  name?: string;
  provider_type?: LlmProviderType;
  endpoint?: string;
  configuration?: { model?: string } & Record<string, unknown>;
  timeout_secs?: number;
}

export function updateLlmProvider(
  teamId: string,
  providerId: string,
  body: UpdateLlmProviderBody
) {
  return http.patch<LlmProvider>(
    `/v1/pm/teams/${teamId}/llm-providers/${providerId}`,
    body
  );
}

/** 替换凭证（替换制轮换：旧凭证留痕，新凭证需重新测试后启用）。 */
export function replaceLlmCredential(
  teamId: string,
  providerId: string,
  body: { expected_revision: number; api_key: string }
) {
  return http.post<LlmProvider>(
    `/v1/pm/teams/${teamId}/llm-providers/${providerId}/credential`,
    body
  );
}

/** 生命周期显式动作（§6.3）：enable 需已有凭证；测试成功≠自动启用。 */
export function setLlmProviderStatus(
  teamId: string,
  providerId: string,
  action: "enable" | "disable" | "archive",
  expectedRevision: number
) {
  return http.post<LlmProvider>(
    `/v1/pm/teams/${teamId}/llm-providers/${providerId}/status`,
    { action, expected_revision: expectedRevision }
  );
}

/** 删除（仅 archived 且无调用记录的实例；执行记录永久保留）。 */
export function deleteLlmProvider(teamId: string, providerId: string) {
  return http.delete<{ deleted: boolean }>(
    `/v1/pm/teams/${teamId}/llm-providers/${providerId}`
  );
}

/**
 * 项目 AI 模型策略（0043）：项目在团队范围内指定 Provider。
 *
 * 解析语义（§10.2/§16）：`provider_id = null` = 未指定 → 继承 Team 首个
 * `enabled` 实例（创建序）；指定后**严格不回退**——绑定的 Provider 即使后续
 * 停用也照用（§10.3），保存允许非 enabled 实例。写走 CAS：
 * `expected_revision` 不一致 → 409 `revision_conflict`（响应含
 * `actual_revision`）。
 */
export interface ProjectAiPolicy {
  project_id: string;
  /** null = 未指定（继承 Team Default；策略行不存在时也是 null） */
  provider_id: string | null;
  /** 绑定实例的当前名称（未指定时 null） */
  provider_name: string | null;
  /** 绑定实例的当前状态（未指定时 null；可≠enabled，严格不回退） */
  provider_status: string | null;
  /** 乐观锁：PUT 必须携带 expected_revision = 当前值（无策略行 = 1） */
  revision: number;
  updated_at: string | null;
}

export function getProjectAiPolicy(projectId: string) {
  return http.get<ProjectAiPolicy>(
    `/v1/pm/projects/${projectId}/ai-model-policies`
  );
}

export interface SetProjectAiPolicyBody {
  /** null = 恢复继承 Team Default（后端删除策略行） */
  provider_id: string | null;
  expected_revision: number;
}

export function setProjectAiPolicy(
  projectId: string,
  body: SetProjectAiPolicyBody
) {
  return http.put<ProjectAiPolicy>(
    `/v1/pm/projects/${projectId}/ai-model-policies`,
    body
  );
}

export interface LlmTestResult {
  ok: boolean;
  latency_ms: number;
  message: string;
}

/** 服务端发起连通性探测（使用已保存的 endpoint/model/key）。 */
export function testLlmProvider(teamId: string, providerId: string) {
  return http.post<LlmTestResult>(
    `/v1/pm/teams/${teamId}/llm-providers/${providerId}/test`
  );
}

// ───────────────────────────────── 模型目录/授权/策略（R3 / B01-04）──────────
//
// B01-04：后端三面齐全（`/v1/pm/teams/{id}/llm-models*`）但前端零调用——
// Provider 启用后 worker 报 general_policy_missing，普通用户走 UI 永远无法
// 激活 AI。以下函数把目录三面接进设置页。

/** 目录模型行（`ai.team_llm_model`）。 */
export interface LlmModelRow {
  id: string;
  provider_id: string;
  provider_name: string;
  model_id: string;
  display_name: string;
  capabilities: string[];
  context_window: number | null;
  enabled: boolean;
  /** human = 人工登记；其余为登记来源标注（不伪造自动发现）。 */
  source: string;
  last_verified_at: string | null;
  /** 乐观锁：update 必须携带 expected_revision。 */
  revision: number;
  created_by: string;
  created_at: string;
  updated_at: string;
}

/** Team 授权行（`ai.team_llm_model_authorization`）。 */
export interface LlmAuthorizationRow {
  model_id: string;
  /** team_wide = 全团队可用（唯一可用值；缺失 = 未授权，解析拒绝）。 */
  authorization: string;
  granted_at: string;
  revision: number;
  /** 撤销影响面（只报告不自动改写）：引用该模型的 Team/项目策略行数。 */
  team_policy_refs: number;
  project_override_refs: number;
}

/** Team 策略行（role_key → model；未配置 Role 继承 general）。 */
export interface LlmPolicyRow {
  role_key: string;
  model_id: string | null;
  model_display_name: string | null;
  provider_name: string | null;
  /** 未配置（无行）→ 解析继承 general。 */
  inherit_general: boolean;
}

export function listLlmModels(
  teamId: string,
  query?: { provider_id?: string; q?: string }
) {
  const params = new URLSearchParams();
  if (query?.provider_id) params.set("provider_id", query.provider_id);
  if (query?.q) params.set("q", query.q);
  const qs = params.toString();
  return http.get<ListResponse<LlmModelRow>>(
    `/v1/pm/teams/${teamId}/llm-models${qs ? `?${qs}` : ""}`
  );
}

export interface CreateLlmModelBody {
  provider_id: string;
  model_id: string;
  display_name: string;
  capabilities?: string[];
  context_window?: number;
}

export function createLlmModel(teamId: string, body: CreateLlmModelBody) {
  return http.post<LlmModelRow>(`/v1/pm/teams/${teamId}/llm-models`, body);
}

export interface UpdateLlmModelBody {
  expected_revision: number;
  display_name?: string;
  capabilities?: string[];
  context_window?: number | null;
  enabled?: boolean;
}

export function updateLlmModel(
  teamId: string,
  modelRowId: string,
  body: UpdateLlmModelBody
) {
  return http.patch<LlmModelRow>(
    `/v1/pm/teams/${teamId}/llm-models/${modelRowId}`,
    body
  );
}

/** Team 范围授权（grant/revoke；PUT 全量 upsert 语义按项生效）。 */
export function upsertLlmAuthorizations(
  teamId: string,
  items: { model_id: string; action: "grant" | "revoke" }[]
) {
  return http.put<{ items: LlmAuthorizationRow[] }>(
    `/v1/pm/teams/${teamId}/llm-model-authorizations`,
    { items }
  );
}

export function listLlmTeamPolicies(teamId: string) {
  return http.get<{ items: LlmPolicyRow[] }>(
    `/v1/pm/teams/${teamId}/llm-model-policies`
  );
}

export function setLlmTeamPolicies(
  teamId: string,
  items: { role_key: string; model_id: string | null }[]
) {
  return http.put<{ items: LlmPolicyRow[] }>(
    `/v1/pm/teams/${teamId}/llm-model-policies`,
    { items }
  );
}

/** Role 词表（与后端 ROLE_KEYS 同源顺序；general = 选模末级兜底）。 */
export const LLM_ROLE_KEYS = [
  "general",
  "ai_pm_lead",
  "requirement_analysis",
  "task_analysis",
  "risk_analysis",
  "architecture_analysis",
  "knowledge_management"
] as const;

/** 模型能力词表（与后端 CAPABILITIES 同源；未知能力=空数组，禁止编造）。 */
export const LLM_CAPABILITIES = [
  "text_generation",
  "reasoning",
  "structured_output",
  "tool_calling",
  "vision",
  "long_context"
] as const;

/** AI 数据面就绪探针（R3/B01-01；项目成员可读，不含任何密钥）。 */
export interface AiReadiness {
  provider_enabled_count: number;
  catalog_model_enabled_count: number;
  team_wide_authorization_count: number;
  general_policy_set: boolean;
  router_enabled: boolean;
  ready: boolean;
}

export function getAiReadiness(projectId: string) {
  return http.get<AiReadiness>(`/v1/pm/projects/${projectId}/ai-readiness`);
}

/** Team 授权清单（只回 team_wide 行；R3/B01-04 补的读取面）。 */
export function listLlmAuthorizations(teamId: string) {
  return http.get<{ items: LlmAuthorizationRow[] }>(
    `/v1/pm/teams/${teamId}/llm-model-authorizations`
  );
}
