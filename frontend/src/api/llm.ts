import { http } from "@/shell/api";
import type { ListResponse } from "./types";

/** 支持的模型供应商（ai.llm_provider.provider 的 CHECK 枚举）。 */
export type LlmProviderId =
  | "deepseek"
  | "openai"
  | "glm"
  | "minimax"
  | "ollama";

/**
 * ai.llm_provider —— 全局模型供应商配置。
 * API Key 明文不回传：只有 `has_api_key` 与末 4 位提示。
 */
export interface LlmProvider {
  provider: LlmProviderId;
  base_url: string;
  model: string;
  has_api_key: boolean;
  api_key_hint: string | null;
  enabled: boolean;
  timeout_secs: number;
  updated_at: string;
}

export function listLlmProviders() {
  return http.get<ListResponse<LlmProvider>>("/v1/pm/llm/providers");
}

export interface UpdateLlmProviderBody {
  base_url: string;
  model: string;
  /** 缺省或空串 = 保持原密钥 */
  api_key?: string;
  /** true = 删除已保存的密钥（本地 Ollama 等无需鉴权场景） */
  clear_api_key?: boolean;
  enabled: boolean;
  timeout_secs: number;
}

export function updateLlmProvider(
  provider: LlmProviderId,
  body: UpdateLlmProviderBody
) {
  return http.put<LlmProvider>(`/v1/pm/llm/providers/${provider}`, body);
}

export interface LlmTestResult {
  ok: boolean;
  latency_ms: number;
  message: string;
}

/** 服务端发起连通性探测（使用已保存的 base_url / model / key）。 */
export function testLlmProvider(provider: LlmProviderId) {
  return http.post<LlmTestResult>(`/v1/pm/llm/providers/${provider}/test`);
}
