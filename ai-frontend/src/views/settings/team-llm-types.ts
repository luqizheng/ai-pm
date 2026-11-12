import type {
  LlmProvider,
  LlmProviderStatus,
  LlmProviderType
} from "@/api/llm";

export type { LlmProvider, LlmProviderStatus, LlmProviderType };

/** 模型静态条目（UI 下拉展示 + API 兜底）。 */
export interface StaticModel {
  id: string;
  name: string;
  desc?: string;
}

/** Provider 的模型来源能力。
 * - api: 有 /models 端点（OpenAI 兼容面），可在 endpoint + api_key 填好后拉取
 * - static: 无统一 /models 端点（如 Gemini），用内置 staticModels
 * - manual: 用户必须手填 model id（自定义 / 本地 Ollama 等）
 */
export type ModelSource = "api" | "static" | "manual";

/** Provider Catalog 条目（UI 下拉、endpoint 预填、Model 字段状态机单源）。 */
export interface ProviderCatalogEntry {
  type: LlmProviderType;
  /** i18n 命名空间 key（settings.team.ai.providerTypes.${value}）。 */
  i18nKey: string;
  /** 新建表单切换该类型时 endpoint 预填值（空串 = 需手填）。 */
  defaultEndpoint: string;
  modelSource: ModelSource;
  /** 仅 modelSource === "api" 有值；OpenAI 兼容 /models 端点路径。 */
  listModelsEndpoint?: string;
  /** api 类型拉 models 是否需要先填 api_key。 */
  requireApiKeyForModels?: boolean;
  /** fallback / manual / static 时使用的内置模型列表。 */
  staticModels: StaticModel[];
}

/** 全量 Provider Catalog —— 前端配置单源。 */
export const PROVIDER_CATALOG: ProviderCatalogEntry[] = [
  {
    type: "deepseek",
    i18nKey: "settings.team.ai.providerTypes.deepseek",
    defaultEndpoint: "https://api.deepseek.com",
    modelSource: "api",
    listModelsEndpoint: "/v1/models",
    requireApiKeyForModels: true,
    staticModels: [
      { id: "deepseek-chat", name: "deepseek-chat", desc: "低延迟通用" },
      { id: "deepseek-reasoner", name: "deepseek-reasoner", desc: "深度推理" }
    ]
  },
  {
    type: "openai",
    i18nKey: "settings.team.ai.providerTypes.openai",
    defaultEndpoint: "https://api.openai.com/v1",
    modelSource: "api",
    listModelsEndpoint: "/models",
    requireApiKeyForModels: true,
    staticModels: [
      { id: "gpt-4o", name: "gpt-4o", desc: "实时多模态" },
      { id: "gpt-4o-mini", name: "gpt-4o-mini", desc: "轻量快速" },
      { id: "o3", name: "o3", desc: "高级推理" },
      { id: "gpt-5", name: "gpt-5", desc: "最新主力" }
    ]
  },
  {
    type: "glm",
    i18nKey: "settings.team.ai.providerTypes.glm",
    defaultEndpoint: "https://open.bigmodel.cn/api/paas/v4",
    modelSource: "api",
    listModelsEndpoint: "/models",
    requireApiKeyForModels: true,
    staticModels: [
      { id: "glm-4.5", name: "glm-4.5", desc: "主力" },
      { id: "glm-4.5-flash", name: "glm-4.5-flash", desc: "低延迟" },
      { id: "glm-4", name: "glm-4", desc: "稳定" },
      { id: "glm-4-air", name: "glm-4-air", desc: "超长上下文" }
    ]
  },
  {
    type: "minimax",
    i18nKey: "settings.team.ai.providerTypes.minimax",
    defaultEndpoint: "https://api.minimax.chat/v1",
    modelSource: "api",
    listModelsEndpoint: "/models",
    requireApiKeyForModels: true,
    staticModels: [
      { id: "abab7.5-chat", name: "abab7.5-chat", desc: "主力" },
      { id: "abab6.5s-chat", name: "abab6.5s-chat", desc: "轻量" },
      { id: "text-01", name: "text-01", desc: "文本生成" }
    ]
  },
  {
    type: "gemini",
    i18nKey: "settings.team.ai.providerTypes.gemini",
    defaultEndpoint: "https://generativelanguage.googleapis.com",
    modelSource: "static",
    staticModels: [
      { id: "gemini-2.0-flash", name: "gemini-2.0-flash", desc: "低延迟" },
      { id: "gemini-2.0-pro", name: "gemini-2.0-pro", desc: "主力" },
      { id: "gemini-1.5-pro", name: "gemini-1.5-pro", desc: "稳定" }
    ]
  },
  {
    // 自托管推理服务：默认端点指本机（AGENTS.md 约定 127.0.0.1，禁 localhost），
    // OpenAI 兼容面 /v1；模型列表来自本地 /models（拉过哪些模型就有哪些），
    // 无需 API Key，列表为空时手动输入 model id（如 llama3:8b）。
    type: "ollama",
    i18nKey: "settings.team.ai.providerTypes.ollama",
    defaultEndpoint: "http://127.0.0.1:11434/v1",
    modelSource: "api",
    listModelsEndpoint: "/models",
    requireApiKeyForModels: false,
    staticModels: []
  },
  {
    type: "openai_compatible",
    i18nKey: "settings.team.ai.providerTypes.openai_compatible",
    defaultEndpoint: "",
    modelSource: "manual",
    staticModels: []
  }
];

// ── 派生工具 ─────────────────────────────────────────────

export function findProvider(
  type: LlmProviderType
): ProviderCatalogEntry | undefined {
  return PROVIDER_CATALOG.find(p => p.type === type);
}

export function defaultEndpointOf(type: LlmProviderType): string {
  return findProvider(type)?.defaultEndpoint ?? "";
}

/** 给 Select 的 options（value = type，label 由 i18n 解析）。 */
export function providerTypeDropdown(i18n: { t: (key: string) => string }) {
  return PROVIDER_CATALOG.map(e => ({
    value: e.type,
    label: i18n.t(e.i18nKey)
  }));
}

// ── 旧常量别名（保留兼容，避免改动 SettingsTeamAiView.vue 之外的引用） ──
export const PROVIDER_ENDPOINT_PRESETS: Partial<
  Record<LlmProviderType, string>
> = Object.fromEntries(
  PROVIDER_CATALOG.filter(e => e.defaultEndpoint).map(e => [
    e.type,
    e.defaultEndpoint
  ])
) as Partial<Record<LlmProviderType, string>>;

// ── Team Provider 卡片表单模型（api_key 不在卡片表单内，单独动作） ─────────────

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
