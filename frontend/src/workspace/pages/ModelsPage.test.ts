// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import ModelsPage from "./ModelsPage.vue";
import { mountShell } from "../../shell/test-utils";

const listLlmProviders = vi.fn();
const updateLlmProvider = vi.fn();
const testLlmProvider = vi.fn();

vi.mock("@/api/llm", () => ({
  listLlmProviders: (...args: unknown[]) => listLlmProviders(...args),
  updateLlmProvider: (...args: unknown[]) => updateLlmProvider(...args),
  testLlmProvider: (...args: unknown[]) => testLlmProvider(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function row(provider: string, model: string) {
  return {
    provider,
    base_url: `https://${provider}.example/v1`,
    model,
    has_api_key: false,
    api_key_hint: null,
    enabled: false,
    timeout_secs: 120,
    updated_at: "2026-10-31T00:00:00Z"
  };
}

const rows = [
  row("deepseek", "deepseek-chat"),
  row("openai", "gpt-4o-mini"),
  row("glm", "glm-4-plus"),
  row("minimax", "MiniMax-Text-01"),
  row("ollama", "qwen2.5:7b")
];

beforeEach(() => {
  vi.clearAllMocks();
  listLlmProviders.mockResolvedValue({ items: rows, total: rows.length });
  updateLlmProvider.mockImplementation(
    (provider: string, body: Record<string, unknown>) => ({
      ...rows.find(item => item.provider === provider),
      ...body,
      has_api_key: Boolean(body.api_key),
      api_key_hint: body.api_key ? "****abcd" : null
    })
  );
  testLlmProvider.mockResolvedValue({
    ok: true,
    latency_ms: 42,
    message: "HTTP 200"
  });
});

describe("ModelsPage（模型配置）", () => {
  it("渲染五家供应商卡片", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    const cards = w.findAll("[data-test='provider-card']");
    expect(cards).toHaveLength(5);
    for (const provider of ["deepseek", "openai", "glm", "minimax", "ollama"]) {
      expect(w.find(`[data-provider='${provider}']`).exists()).toBe(true);
    }
    expect(w.text()).toContain("DeepSeek");
    expect(w.text()).toContain("Ollama");
  });

  it("保存时提交当前表单字段（base_url / model / enabled / timeout）", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    const card = w.find("[data-provider='deepseek']");
    await card.find("[data-test='save']").trigger("click");
    await flushPromises();
    expect(updateLlmProvider).toHaveBeenCalledWith(
      "deepseek",
      expect.objectContaining({
        base_url: "https://deepseek.example/v1",
        model: "deepseek-chat",
        enabled: false,
        timeout_secs: 120
      })
    );
  });

  it("测试连接后展示延迟结果", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    const card = w.find("[data-provider='glm']");
    await card.find("[data-test='test']").trigger("click");
    await flushPromises();
    expect(testLlmProvider).toHaveBeenCalledWith("glm");
    expect(w.find("[data-test='test-result']").text()).toContain("42");
  });

  it("超时超出 1–600 时禁用保存", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    const card = w.find("[data-provider='openai']");
    const timeoutInput = card.find("input[type='number']");
    await timeoutInput.setValue("999");
    await flushPromises();
    expect(
      card.find("[data-test='save']").attributes("disabled")
    ).toBeDefined();
    expect(card.text()).toContain("1–600");
  });
});
