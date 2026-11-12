/**
 * Provider 模型列表加载 — 状态机 + 缓存 + 降级。
 *
 * 适配器边界：UI 层只拿到统一的 CombinedModel[]，
 * 供应商 /models 端点的鉴权、路径、返回结构全收敛在这里。
 */
import { ref, watch, computed } from "vue";
import type {
  LlmProviderType,
  ProviderCatalogEntry,
  StaticModel
} from "./team-llm-types";

export type ModelLoadStatus =
  | "idle"        // api 类型但 endpoint/key 未填好 → 占位
  | "loading"     // 正在请求
  | "success"     // API 成功（可能带 staticModels 合并）
  | "fallback"    // API 失败 → 降级到 staticModels
  | "empty"       // API 成功但空 + 无 staticModels
  | "unsupported" // manual / static 类型，不请求

export interface CombinedModel extends StaticModel {
  /** true = 来自 Provider /models；false = 内置 staticModels 推荐项 */
  fromApi: boolean;
}

const STATUS_GROUP = "来自 Provider";
const STATIC_GROUP = "内置推荐";

// ── 内存缓存：key = `${type}|${endpoint}|${api_key_hash}` ──────────────
const cache = new Map<string, CombinedModel[]>();

function cacheKey(
  type: LlmProviderType,
  endpoint: string,
  apiKey: string
): string {
  const keyHash = apiKey
    ? simpleHash(apiKey)
    : "";
  return `${type}|${endpoint}|${keyHash}`;
}

// 简单非加密 hash（只用于区分缓存键，不涉及安全）。
function simpleHash(s: string): string {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0).toString(36);
}

// ── OpenAI 兼容 /models 请求 ──────────────────────────────────────────

interface OpenAIListModelsResponse {
  data?: Array<{ id: string }>;
}

const REQUEST_TIMEOUT_MS = 8000;

async function fetchOpenAiModels(
  endpoint: string,
  listPath: string,
  apiKey: string,
  signal: AbortSignal
): Promise<string[]> {
  const url = endpoint.replace(/\/+$/, "") + listPath;
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), REQUEST_TIMEOUT_MS);
  // 让 timeout signal 也能被外部 abort 打断
  signal.addEventListener("abort", () => ctrl.abort());

  try {
    const resp = await fetch(url, {
      method: "GET",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        Accept: "application/json"
      },
      signal: ctrl.signal
    });
    if (!resp.ok) {
      throw new Error(`HTTP ${resp.status}`);
    }
    const body = (await resp.json()) as OpenAIListModelsResponse;
    return (body.data ?? []).map(m => m.id).filter(Boolean);
  } finally {
    clearTimeout(timer);
  }
}

// ── 合并 + 去重 ────────────────────────────────────────────────────────

/** API 优先；同 id 时 staticModels 丢弃（去重）。 */
function combine(
  apiIds: string[],
  staticModels: StaticModel[]
): CombinedModel[] {
  const seen = new Set<string>();
  const out: CombinedModel[] = [];
  for (const id of apiIds) {
    seen.add(id);
    out.push({ id, name: id, fromApi: true });
  }
  for (const s of staticModels) {
    if (!seen.has(s.id)) {
      out.push({ ...s, fromApi: false });
    }
  }
  return out;
}

// ── composable ────────────────────────────────────────────────────────

export interface UseModelSourceOptions {
  /** 强制忽略缓存重取（"刷新模型"按钮）。 */
  force?: boolean;
}

export interface UseModelSourceReturn {
  /** 合并后的列表，按来源分组消费。 */
  options: import("vue").ComputedRef<
    Array<{ group?: string; items: CombinedModel[] }>
  >;
  status: import("vue").Ref<ModelLoadStatus>;
  /** 失败或降级时的提示文案（i18n key 或硬编码）。 */
  hint: import("vue").Ref<string | null>;
  /** 手动触发刷新（忽略缓存）。 */
  refresh: () => void;
  /** 当前是否可请求（api 类型 + endpoint + key 齐）。 */
  canLoad: import("vue").ComputedRef<boolean>;
}

export function useModelSource(
  provider: import("vue").ComputedRef<ProviderCatalogEntry | undefined>,
  endpoint: import("vue").Ref<string>,
  apiKey: import("vue").Ref<string>
): UseModelSourceReturn {
  const status = ref<ModelLoadStatus>("idle");
  const hint = ref<string | null>(null);
  const models = ref<CombinedModel[]>([]);
  let controller: AbortController | null = null;
  let refreshSeq = 0;

  const canLoad = computed(() => {
    const p = provider.value;
    if (!p) return false;
    if (p.modelSource !== "api") return false;
    if (!endpoint.value.trim()) return false;
    if (p.requireApiKeyForModels && !apiKey.value.trim()) return false;
    return true;
  });

  const options = computed(() => {
    // 骨架态 → 返回 skeleton 占位项
    if (status.value === "loading") {
      return [{ group: STATUS_GROUP, items: skeletonModels(6) }];
    }
    // unsupported / static → 只展示 staticModels（或手动输入提示）
    if (status.value === "unsupported") {
      const staticModels = provider.value?.staticModels ?? [];
      if (staticModels.length === 0) {
        return [];
      }
      return [{ group: STATIC_GROUP, items: staticModels.map(m => ({ ...m, fromApi: false as const })) }];
    }
    // fallback / empty / idle（无 API 数据时用 staticModels）
    if (status.value === "fallback" || status.value === "empty" || status.value === "idle") {
      const staticModels = provider.value?.staticModels ?? [];
      if (staticModels.length === 0) return [];
      return [{ group: STATIC_GROUP, items: staticModels.map(m => ({ ...m, fromApi: false as const })) }];
    }
    // success → 分两组
    const api = models.value.filter(m => m.fromApi);
    const staticList = models.value.filter(m => !m.fromApi);
    const groups: Array<{ group?: string; items: CombinedModel[] }> = [];
    if (api.length) groups.push({ group: STATUS_GROUP, items: api });
    if (staticList.length) groups.push({ group: STATIC_GROUP, items: staticList });
    return groups;
  });

  function skeletonModels(n: number): CombinedModel[] {
    return Array.from({ length: n }, (_, i) => ({
      id: `__skeleton_${i}`,
      name: "",
      fromApi: false
    }));
  }

  function abort() {
    controller?.abort();
    controller = null;
  }

  async function load(force = false) {
    const p = provider.value;
    if (!p) return;

    // manual / static 类型 → 不请求
    if (p.modelSource !== "api") {
      status.value = "unsupported";
      hint.value = null;
      models.value = p.staticModels.map(m => ({ ...m, fromApi: false }));
      return;
    }

    // api 类型但未填 endpoint/key → idle
    if (!endpoint.value.trim()) {
      status.value = "idle";
      hint.value = "填写 API 地址后自动加载模型";
      models.value = p.staticModels.map(m => ({ ...m, fromApi: false }));
      return;
    }
    if (p.requireApiKeyForModels && !apiKey.value.trim()) {
      status.value = "idle";
      hint.value = "填写 API Key 后自动加载模型";
      models.value = p.staticModels.map(m => ({ ...m, fromApi: false }));
      return;
    }

    const ckey = cacheKey(p.type, endpoint.value, apiKey.value);
    if (!force && cache.has(ckey)) {
      models.value = cache.get(ckey)!;
      if (models.value.length === 0) {
        status.value = "empty";
        hint.value = `${STATUS_GROUP} 返回为空，请手动输入 model id`;
      } else {
        status.value = "success";
        hint.value = null;
      }
      return;
    }

    // 取消上一个请求（切换 provider 时）
    abort();
    const seq = ++refreshSeq;
    const ctrl = new AbortController();
    controller = ctrl;
    status.value = "loading";
    hint.value = null;

    try {
      const apiIds = await fetchOpenAiModels(
        endpoint.value.trim(),
        p.listModelsEndpoint!,
        apiKey.value.trim(),
        ctrl.signal
      );
      if (seq !== refreshSeq) return; // 被新请求覆盖

      const combined = combine(apiIds, p.staticModels);
      cache.set(ckey, combined);
      models.value = combined;

      if (apiIds.length === 0 && combined.length === 0) {
        status.value = "empty";
        hint.value = "该 Provider 未返回模型，请手动输入 model id";
      } else {
        status.value = "success";
        hint.value = null;
      }
    } catch (e) {
      if (ctrl.signal.aborted) return; // 正常取消，不降级
      if (seq !== refreshSeq) return;

      // 静默降级 → fallback
      const staticOnly = p.staticModels.map(m => ({ ...m, fromApi: false as const }));
      models.value = staticOnly;
      if (staticOnly.length === 0) {
        status.value = "empty";
        hint.value =
          "无法从 Provider 获取模型列表，请手动输入 model id";
      } else {
        status.value = "fallback";
        hint.value =
          "无法从 Provider 获取模型列表，已切换为内置列表，可手动输入";
      }
    }
  }

  function refresh() {
    const pType = provider.value?.type;
    if (pType) {
      cache.delete(cacheKey(pType, endpoint.value, apiKey.value));
    }
    load(true);
  }

  // 监听关键变化，自动触发请求
  watch(
    [provider, endpoint, apiKey],
    () => {
      // endpoint / key 变化时 debounce
      loadDebounced();
    },
    { deep: false }
  );

  // provider 类型切换时立即 abort + 清空旧值
  let lastType: string | undefined;
  watch(
    () => provider.value?.type,
    (t) => {
      if (t !== lastType) {
        abort();
        lastType = t;
        // 切换 provider 时立即重新判定状态
        queueMicrotask(() => load());
      }
    }
  );

  // debounce 500ms
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  function loadDebounced() {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => load(), 500);
  }

  // 初始判定
  queueMicrotask(() => load());

  return { options, status, hint, refresh, canLoad };
}
