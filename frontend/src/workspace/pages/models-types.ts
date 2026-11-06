import type {
  LlmProvider,
  LlmProviderStatus,
  LlmProviderType,
  LlmTestResult
} from "@/api/llm";

/** Provider 卡片的本地表单状态（api_key 为空表示保持原凭证）。 */
export interface ProviderForm {
  name: string;
  provider_type: LlmProviderType;
  endpoint: string;
  /** configuration.model（Phase 3 模型目录前的过渡字段）。 */
  model: string;
  api_key: string;
  timeout: string;
}

export interface ProviderItem {
  data: LlmProvider;
  label: string;
  form: ProviderForm;
  testResult?: LlmTestResult;
}

/** 卡片 → 页面的动作载荷（含卡片本地表单副本）。 */
export interface ProviderAction {
  providerId: string;
  label: string;
  revision: number;
  form: ProviderForm;
}

export const PROVIDER_TYPES: LlmProviderType[] = [
  "openai",
  "anthropic",
  "gemini",
  "deepseek",
  "openai_compatible"
];

export function timeoutOf(form: ProviderForm): number {
  const value = Number(form.timeout);
  return Number.isFinite(value) ? Math.trunc(value) : 0;
}

export function invalidTimeout(form: ProviderForm): boolean {
  const value = timeoutOf(form);
  return value < 1 || value > 600;
}

export function statusBadgeVariant(
  status: LlmProviderStatus
): "success" | "danger" | "neutral" {
  if (status === "enabled" || status === "connected") return "success";
  if (status === "failed") return "danger";
  return "neutral";
}
