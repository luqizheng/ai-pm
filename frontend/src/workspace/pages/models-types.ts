import type { LlmProvider } from "@/api/llm";
import type { LlmTestResult } from "@/api/llm";

/** 模型配置卡片的本地表单状态（api_key 为空表示保持原密钥）。 */
export interface ProviderForm {
  base_url: string;
  model: string;
  api_key: string;
  timeout: string;
  enabled: boolean;
}

export interface ProviderItem {
  data: LlmProvider;
  label: string;
  form: ProviderForm;
  testResult?: LlmTestResult;
}

/** 卡片 → 页面的保存/清除动作载荷（含卡片本地表单副本）。 */
export interface ProviderAction {
  provider: LlmProvider["provider"];
  label: string;
  form: ProviderForm;
}

export function timeoutOf(form: ProviderForm): number {
  const value = Number(form.timeout);
  return Number.isFinite(value) ? Math.trunc(value) : 0;
}

export function invalidTimeout(form: ProviderForm): boolean {
  const value = timeoutOf(form);
  return value < 1 || value > 600;
}
