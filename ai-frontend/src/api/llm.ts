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
  const me = await http.get<{ tenant_id: string }>("/v1/pm/me");
  return me.tenant_id;
}

export function listLlmProviders(tenantId: string) {
  return http.get<ListResponse<LlmProvider>>(
    `/v1/pm/tenants/${tenantId}/llm-providers`
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

export function createLlmProvider(
  tenantId: string,
  body: CreateLlmProviderBody
) {
  return http.post<LlmProvider>(
    `/v1/pm/tenants/${tenantId}/llm-providers`,
    body
  );
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
  tenantId: string,
  providerId: string,
  body: UpdateLlmProviderBody
) {
  return http.patch<LlmProvider>(
    `/v1/pm/tenants/${tenantId}/llm-providers/${providerId}`,
    body
  );
}

/** 替换凭证（替换制轮换：旧凭证留痕，新凭证需重新测试后启用）。 */
export function replaceLlmCredential(
  tenantId: string,
  providerId: string,
  body: { expected_revision: number; api_key: string }
) {
  return http.post<LlmProvider>(
    `/v1/pm/tenants/${tenantId}/llm-providers/${providerId}/credential`,
    body
  );
}

/** 生命周期显式动作（§6.3）：enable 需已有凭证；测试成功≠自动启用。 */
export function setLlmProviderStatus(
  tenantId: string,
  providerId: string,
  action: "enable" | "disable" | "archive",
  expectedRevision: number
) {
  return http.post<LlmProvider>(
    `/v1/pm/tenants/${tenantId}/llm-providers/${providerId}/status`,
    { action, expected_revision: expectedRevision }
  );
}

/** 删除（仅 archived 且无调用记录的实例；执行记录永久保留）。 */
export function deleteLlmProvider(tenantId: string, providerId: string) {
  return http.delete<{ deleted: boolean }>(
    `/v1/pm/tenants/${tenantId}/llm-providers/${providerId}`
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
export function testLlmProvider(tenantId: string, providerId: string) {
  return http.post<LlmTestResult>(
    `/v1/pm/tenants/${tenantId}/llm-providers/${providerId}/test`
  );
}
