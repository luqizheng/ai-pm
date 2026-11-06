// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import SettingsTeamAiView from "./SettingsTeamAiView.vue";

/**
 * 团队设置 — AI 配置（0041 Team LLM Provider）页面契约：
 *   - 挂载即解析租户并拉取 Provider 列表；
 *   - 落地页有团队区入口（路由可达）；
 *   - 新建表单提交创建端点；
 *   - 卡片动作（保存/测试/启停）调用对应 API。
 */
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

function row(overrides: Record<string, unknown> = {}) {
  return {
    id: "id-1",
    name: "DeepSeek 主力",
    provider_type: "deepseek",
    endpoint: "https://api.deepseek.com/v1",
    configuration: { model: "deepseek-chat" },
    status: "configured",
    timeout_secs: 120,
    revision: 1,
    has_credential: true,
    credential_hint: "****abcd",
    last_tested_at: null,
    created_by: "usr",
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-01T00:00:00Z",
    ...overrides
  };
}

const mountedWrappers: VueWrapper[] = [];

async function mountAt(path = "/settings/team/ai-providers") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(SettingsTeamAiView, { global: { plugins: [router] } });
  mountedWrappers.push(wrapper);
  return wrapper;
}

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

beforeEach(() => {
  vi.clearAllMocks();
  resolveTenantId.mockResolvedValue(TENANT_ID);
  listLlmProviders.mockResolvedValue({
    items: [row()],
    total: 1
  });
  updateLlmProvider.mockImplementation(
    (_t: string, id: string, body: Record<string, unknown>) => ({
      ...row(),
      ...body,
      revision: (body.expected_revision as number) + 1
    })
  );
  testLlmProvider.mockResolvedValue({
    ok: true,
    latency_ms: 42,
    message: "HTTP 200"
  });
  setLlmProviderStatus.mockImplementation(
    (_t: string, id: string, action: string) =>
      row({
        status: action === "enable" ? "enabled" : action === "disable" ? "disabled" : "archived"
      })
  );
});

describe("SettingsTeamAiView（团队 AI 配置）", () => {
  it("路由可达：/settings/team/ai-providers 注册进 buildRoutes", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push("/settings/team/ai-providers");
    await router.isReady();
    expect(router.currentRoute.value.path).toBe("/settings/team/ai-providers");
  });

  it("挂载即解析租户并渲染 Provider 卡片", async () => {
    const w = await mountAt();
    await flushPromises();
    expect(resolveTenantId).toHaveBeenCalled();
    expect(listLlmProviders).toHaveBeenCalledWith(TENANT_ID);
    expect(w.findAll("[data-test='provider-card']")).toHaveLength(1);
    expect(w.text()).toContain("DeepSeek 主力");
    expect(w.text()).toContain("已配置（****abcd）");
  });

  it("保存卡片表单走 PATCH（expected_revision）", async () => {
    const w = await mountAt();
    await flushPromises();
    await w.find("[data-provider='id-1'] [data-test='save']").trigger("click");
    await flushPromises();
    expect(updateLlmProvider).toHaveBeenCalledWith(
      TENANT_ID,
      "id-1",
      expect.objectContaining({
        expected_revision: 1,
        endpoint: "https://api.deepseek.com/v1",
        configuration: { model: "deepseek-chat" }
      })
    );
  });

  it("测试连接展示延迟结果", async () => {
    const w = await mountAt();
    await flushPromises();
    await w.find("[data-provider='id-1'] [data-test='test']").trigger("click");
    await flushPromises();
    expect(testLlmProvider).toHaveBeenCalledWith(TENANT_ID, "id-1");
    expect(w.find("[data-test='test-result']").text()).toContain("42");
  });

  it("新建 Provider 提交创建端点", async () => {
    createLlmProvider.mockResolvedValue(row({ id: "id-2", name: "新实例" }));
    const w = await mountAt();
    await flushPromises();
    await w.find("[data-test='add-provider']").trigger("click");
    await w.find("[data-test='create-name'] input").setValue("新实例");
    await w.find("[data-test='create-endpoint'] input").setValue("https://api.example.com/v1");
    await w.find("[data-test='create-model'] input").setValue("test-model");
    await w.find("[data-test='create-key'] input").setValue("sk-secret-9999abcd");
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

  it("configured 卡片可启用（显式动作）", async () => {
    const w = await mountAt();
    await flushPromises();
    await w.find("[data-provider='id-1'] [data-test='enable']").trigger("click");
    await flushPromises();
    expect(setLlmProviderStatus).toHaveBeenCalledWith(
      TENANT_ID,
      "id-1",
      "enable",
      1
    );
  });
});
