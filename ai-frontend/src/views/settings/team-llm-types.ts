import type {
  LlmProvider,
  LlmProviderStatus,
  LlmProviderType
} from "@/api/llm";

export type { LlmProvider, LlmProviderStatus, LlmProviderType };

/** Team Provider 卡片表单模型（api_key 不在卡片表单内，单独动作）。 */
export interface ProviderCardForm {
  name: string;
  endpoint: string;
  model: string;
  timeout: string;
}

export function cardFormOf(row: LlmProvider): ProviderCardForm {
  return {
    name: row.name,
    endpoint: row.endpoint,
    model: String(row.configuration?.model ?? ""),
    timeout: String(row.timeout_secs)
  };
}

export function timeoutOf(form: ProviderCardForm): number {
  const value = Number(form.timeout);
  return Number.isFinite(value) ? Math.trunc(value) : 0;
}

export function invalidTimeout(form: ProviderCardForm): boolean {
  const value = timeoutOf(form);
  return value < 1 || value > 600;
}

export function configurationOf(form: ProviderCardForm) {
  const model = form.model.trim();
  return model ? { model } : {};
}
