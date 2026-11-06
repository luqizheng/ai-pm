// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import ModelsPage from "./ModelsPage.vue";
import { mountShell } from "../../shell/test-utils";

const TENANT_ID = "00000000-0000-0000-0000-000000000010";

const resolveTenantId = vi.fn();
const listLlmProviders = vi.fn();
const createLlmProvider = vi.fn();
const updateLlmProvider = vi.fn();
const replaceLlmCredential = vi.fn();
const setLlmProviderStatus = vi.fn();
const testLlmProvider = vi.fn();
const deleteLlmProvider = vi.fn();

vi.mock("@/api/llm", () => ({
  resolveTenantId: (...args: unknown[]) => resolveTenantId(...args),
  listLlmProviders: (...args: unknown[]) => listLlmProviders(...args),
  createLlmProvider: (...args: unknown[]) => createLlmProvider(...args),
  updateLlmProvider: (...args: unknown[]) => updateLlmProvider(...args),
  replaceLlmCredential: (...args: unknown[]) => replaceLlmCredential(...args),
  setLlmProviderStatus: (...args: unknown[]) => setLlmProviderStatus(...args),
  testLlmProvider: (...args: unknown[]) => testLlmProvider(...args),
  deleteLlmProvider: (...args: unknown[]) => deleteLlmProvider(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function row(overrides: Record<string, unknown>) {
  return {
    id: "id-1",
    name: "DeepSeek 主力",
    provider_type: "deepseek",
    endpoint: "https://api.deepseek.com/v1",
    configuration: { model: "deepseek-chat" },
    status: "configured",
    timeout_secs: 120,
    revision: 1,
    has_credential: false,
    credential_hint: null,
    last_tested_at: null,
    created_by: "usr",
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-01T00:00:00Z",
    ...overrides
  };
}

const rows = [
  row({ id: "id-1", name: "DeepSeek 主力", provider_type: "deepseek", status: "configured" }),
  row({
    id: "id-2",
    name: "本地 Ollama",
    provider_type: "openai_compatible",
    endpoint: "http://127.0.0.1:11434/v1",
    status: "not_configured"
  })
];

beforeEach(() => {
  vi.clearAllMocks();
  resolveTenantId.mockResolvedValue(TENANT_ID);
  listLlmProviders.mockResolvedValue({ items: rows, total: rows.length });
  updateLlmProvider.mockImplementation(
    (_tenant: string, id: string, body: Record<string, unknown>) => ({
      ...rows.find(item => item.id === id),
      ...body,
      revision: (body.expected_revision as number) + 1
    })
  );
  testLlmProvider.mockResolvedValue({
    ok: true,
    latency_ms: 42,
    message: "HTTP 200"
  });
});

describe("ModelsPage（模型配置，0041 Team LLM Provider）", () => {
  it("解析租户并渲染 Provider 实例卡片", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    expect(resolveTenantId).toHaveBeenCalled();
    expect(listLlmProviders).toHaveBeenCalledWith(TENANT_ID);
    const cards = w.findAll("[data-test='provider-card']");
    expect(cards).toHaveLength(2);
    expect(w.text()).toContain("DeepSeek 主力");
    expect(w.text()).toContain("本地 Ollama");
  });

  it("保存时提交 PATCH（expected_revision + 全字段）", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    const card = w.find("[data-provider='id-1']");
    await card.find("[data-test='save']").trigger("click");
    await flushPromises();
    expect(updateLlmProvider).toHaveBeenCalledWith(
      TENANT_ID,
      "id-1",
      expect.objectContaining({
        expected_revision: 1,
        name: "DeepSeek 主力",
        provider_type: "deepseek",
        endpoint: "https://api.deepseek.com/v1",
        configuration: { model: "deepseek-chat" },
        timeout_secs: 120
      })
    );
  });

  it("测试连接后展示延迟结果", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    const card = w.find("[data-provider='id-2']");
    await card.find("[data-test='test']").trigger("click");
    await flushPromises();
    expect(testLlmProvider).toHaveBeenCalledWith(TENANT_ID, "id-2");
    expect(w.find("[data-test='test-result']").text()).toContain("42");
  });

  it("新建 Provider 提交创建端点（含凭证）", async () => {
    createLlmProvider.mockResolvedValue(
      row({ id: "id-3", name: "新实例", status: "configured", revision: 2 })
    );
    const w = mountShell(ModelsPage);
    await flushPromises();
    await w.find("[data-test='add-provider']").trigger("click");
    await w.find("[data-test='create-name']").setValue("新实例");
    await w.find("[data-test='create-endpoint']").setValue("https://api.example.com/v1");
    await w.find("[data-test='create-model']").setValue("test-model");
    await w.find("[data-test='create-key']").setValue("sk-secret-9999abcd");
    await w.find("[data-test='create-submit']").trigger("click");
    await flushPromises();
    expect(createLlmProvider).toHaveBeenCalledWith(
      TENANT_ID,
      expect.objectContaining({
        name: "新实例",
        endpoint: "https://api.example.com/v1",
        configuration: { model: "test-model" },
        api_key: "sk-secret-9999abcd"
      })
    );
  });

  it("超时超出 1–600 时禁用保存", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    const card = w.find("[data-provider='id-1']");
    const timeoutInput = card.find("input[type='number']");
    await timeoutInput.setValue("999");
    await flushPromises();
    expect(
      card.find("[data-test='save']").attributes("disabled")
    ).toBeDefined();
    expect(card.text()).toContain("1–600");
  });

  it("not_configured 实例有启用入口，archived 实例有删除入口", async () => {
    const w = mountShell(ModelsPage);
    await flushPromises();
    const card1 = w.find("[data-provider='id-1']");
    expect(card1.find("[data-test='enable']").exists()).toBe(true);
    expect(card1.find("[data-test='delete']").exists()).toBe(false);
  });
});
