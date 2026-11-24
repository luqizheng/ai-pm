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
// R3/B01-04：目录三面 + 授权清单（TeamModelCatalog 子组件消费）。
const listLlmModels = vi.fn();
const createLlmModel = vi.fn();
const updateLlmModel = vi.fn();
const listLlmAuthorizations = vi.fn();
const upsertLlmAuthorizations = vi.fn();
const listLlmTeamPolicies = vi.fn();
const setLlmTeamPolicies = vi.fn();

vi.mock("@/api/llm", () => ({
  resolveTenantId: (...args: unknown[]) => resolveTenantId(...args),
  listLlmProviders: (...args: unknown[]) => listLlmProviders(...args),
  createLlmProvider: (...args: unknown[]) => createLlmProvider(...args),
  updateLlmProvider: (...args: unknown[]) => updateLlmProvider(...args),
  replaceLlmCredential: (...args: unknown[]) => replaceLlmCredential(...args),
  setLlmProviderStatus: (...args: unknown[]) => setLlmProviderStatus(...args),
  testLlmProvider: (...args: unknown[]) => testLlmProvider(...args),
  deleteLlmProvider: (...args: unknown[]) => deleteLlmProvider(...args),
  listLlmModels: (...args: unknown[]) => listLlmModels(...args),
  createLlmModel: (...args: unknown[]) => createLlmModel(...args),
  updateLlmModel: (...args: unknown[]) => updateLlmModel(...args),
  listLlmAuthorizations: (...args: unknown[]) => listLlmAuthorizations(...args),
  upsertLlmAuthorizations: (...args: unknown[]) =>
    upsertLlmAuthorizations(...args),
  listLlmTeamPolicies: (...args: unknown[]) => listLlmTeamPolicies(...args),
  setLlmTeamPolicies: (...args: unknown[]) => setLlmTeamPolicies(...args),
  // 词表常量（组件 import 的值）。
  LLM_ROLE_KEYS: [
    "general",
    "ai_pm_lead",
    "requirement_analysis",
    "task_analysis",
    "risk_analysis",
    "architecture_analysis",
    "knowledge_management"
  ],
  LLM_CAPABILITIES: [
    "text_generation",
    "reasoning",
    "structured_output",
    "tool_calling",
    "vision",
    "long_context"
  ]
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
  const wrapper = mountShell(SettingsTeamAiView, {
    global: { plugins: [router] }
  });
  mountedWrappers.push(wrapper);
  return wrapper;
}

/** reka-ui Select 浮层经 portal 挂到 body，需要额外宏任务才能渲染完成 */
async function flushPortal() {
  await flushPromises();
  await new Promise(resolve => setTimeout(resolve, 0));
  await flushPromises();
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
        status:
          action === "enable"
            ? "enabled"
            : action === "disable"
              ? "disabled"
              : "archived"
      })
  );
  // R3/B01-04：目录面默认空态（行内启用 Provider 以驱动目录表单下拉）。
  listLlmProviders.mockResolvedValue({ items: [row()], total: 1 });
  listLlmModels.mockResolvedValue({ items: [], total: 0 });
  listLlmTeamPolicies.mockResolvedValue({ items: [] });
  listLlmAuthorizations.mockResolvedValue({ items: [] });
  createLlmModel.mockImplementation(
    (_t: string, body: Record<string, unknown>) => modelRow(body)
  );
  upsertLlmAuthorizations.mockResolvedValue({
    items: [
      {
        model_id: "mrow-1",
        authorization: "team_wide",
        granted_at: "2026-11-24T00:00:00Z",
        revision: 1,
        team_policy_refs: 0,
        project_override_refs: 0
      }
    ]
  });
  setLlmTeamPolicies.mockImplementation((_t: string, items: unknown) => ({
    items
  }));
});

function modelRow(overrides: Record<string, unknown> = {}) {
  return {
    id: "mrow-1",
    provider_id: "id-1",
    provider_name: "DeepSeek 主力",
    model_id: "deepseek-chat",
    display_name: "DeepSeek Chat",
    capabilities: ["text_generation"],
    context_window: null,
    enabled: true,
    source: "human",
    last_verified_at: null,
    revision: 1,
    created_by: "usr",
    created_at: "2026-11-24T00:00:00Z",
    updated_at: "2026-11-24T00:00:00Z",
    ...overrides
  };
}

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
    await w
      .find("[data-test='create-endpoint'] input")
      .setValue("https://api.example.com/v1");
    await w.find("[data-test='create-model'] input").setValue("test-model");
    await w
      .find("[data-test='create-key'] input")
      .setValue("sk-secret-9999abcd");
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

  it("选择内置类型自动预填端点；名称留空时按类型+模型自动命名", async () => {
    createLlmProvider.mockResolvedValue(
      row({ id: "id-3", name: "DeepSeek-deepseek-chat" })
    );
    const w = await mountAt();
    await flushPromises();
    await w.find("[data-test='add-provider']").trigger("click");

    // 打开类型下拉（reka-ui Select 触发器）并选择 DeepSeek
    await w.find("[role='combobox']").trigger("pointerdown");
    await flushPortal();
    const option = Array.from(
      document.body.querySelectorAll("[role='option']")
    ).find(el => el.textContent?.includes("DeepSeek"));
    expect(option, "DeepSeek 选项应出现在下拉浮层").toBeTruthy();
    option!.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    await flushPortal();

    // 内置类型预填默认端点
    const endpointInput = w.find("[data-test='create-endpoint'] input")
      .element as HTMLInputElement;
    expect(endpointInput.value).toBe("https://api.deepseek.com");

    // 名称留空 + 填模型 → 提交时自动命名「类型-模型」
    await w.find("[data-test='create-model'] input").setValue("deepseek-chat");
    await flushPromises();
    expect(
      (w.find("[data-test='create-name'] input").element as HTMLInputElement)
        .placeholder
    ).toBe("DeepSeek-deepseek-chat");
    await w.find("[data-test='create-submit']").trigger("click");
    await flushPromises();
    expect(createLlmProvider).toHaveBeenCalledWith(
      TENANT_ID,
      expect.objectContaining({
        name: "DeepSeek-deepseek-chat",
        endpoint: "https://api.deepseek.com"
      })
    );
  });

  it("configured 卡片可启用（显式动作）", async () => {
    const w = await mountAt();
    await flushPromises();
    await w
      .find("[data-provider='id-1'] [data-test='enable']")
      .trigger("click");
    await flushPromises();
    expect(setLlmProviderStatus).toHaveBeenCalledWith(
      TENANT_ID,
      "id-1",
      "enable",
      1
    );
  });
});

// ────────────────────────────── R3/B01-04：模型目录/授权/策略区 ──────────

describe("SettingsTeamAiView（模型目录区）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resolveTenantId.mockResolvedValue(TENANT_ID);
    listLlmProviders.mockResolvedValue({
      items: [row({ status: "enabled" })],
      total: 1
    });
    listLlmTeamPolicies.mockResolvedValue({ items: [] });
    listLlmAuthorizations.mockResolvedValue({ items: [] });
    createLlmModel.mockImplementation(
      (_t: string, body: Record<string, unknown>) => modelRow(body)
    );
    upsertLlmAuthorizations.mockResolvedValue({
      items: [
        {
          model_id: "mrow-1",
          authorization: "team_wide",
          granted_at: "2026-11-24T00:00:00Z",
          revision: 1,
          team_policy_refs: 0,
          project_override_refs: 0
        }
      ]
    });
    setLlmTeamPolicies.mockImplementation((_t: string, items: unknown) => ({
      items
    }));
  });

  it("渲染目录行与真实授权态（team_wide → 显示撤销钮）", async () => {
    listLlmModels.mockResolvedValue({
      items: [modelRow()],
      total: 1
    });
    listLlmAuthorizations.mockResolvedValue({
      items: [
        {
          model_id: "mrow-1",
          authorization: "team_wide",
          granted_at: "2026-11-24T00:00:00Z",
          revision: 1,
          team_policy_refs: 0,
          project_override_refs: 0
        }
      ]
    });
    const w = await mountAt();
    await flushPromises();

    expect(listLlmModels).toHaveBeenCalledWith(TENANT_ID);
    expect(w.find("[data-test='model-catalog']").exists()).toBe(true);
    expect(w.find("[data-test='catalog-row-deepseek-chat']").exists()).toBe(
      true
    );
    // 已授权 → 撤销钮可见、授权钮不渲染。
    expect(w.find("[data-test='catalog-revoke-deepseek-chat']").exists()).toBe(
      true
    );
    expect(w.find("[data-test='catalog-grant-deepseek-chat']").exists()).toBe(
      false
    );
  });

  it("未授权行显示授权钮；撤销调真实命令", async () => {
    listLlmModels.mockResolvedValue({ items: [modelRow()], total: 1 });
    const w = await mountAt();
    await flushPromises();

    expect(w.find("[data-test='catalog-grant-deepseek-chat']").exists()).toBe(
      true
    );
    await w
      .find("[data-test='catalog-grant-deepseek-chat']")
      .trigger("click");
    await flushPromises();
    expect(upsertLlmAuthorizations).toHaveBeenCalledWith(TENANT_ID, [
      { model_id: "mrow-1", action: "grant" }
    ]);
  });

  it("登记模型提交 createLlmModel（provider/model_id 必填）", async () => {
    const w = await mountAt();
    await flushPromises();

    // Provider 下拉（reka-ui portal）：选「DeepSeek 主力」。
    await w
      .find("[data-test='model-catalog'] [role='combobox']")
      .trigger("pointerdown");
    await flushPortal();
    const option = Array.from(
      document.body.querySelectorAll("[role='option']")
    ).find(el => el.textContent?.includes("DeepSeek 主力"));
    option!.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    await flushPortal();

    await w
      .find("[data-test='catalog-add-model-id'] input")
      .setValue("deepseek-reasoner");
    await w.find("[data-test='catalog-add-submit']").trigger("click");
    await flushPromises();

    expect(createLlmModel).toHaveBeenCalledWith(
      TENANT_ID,
      expect.objectContaining({
        provider_id: "id-1",
        model_id: "deepseek-reasoner",
        display_name: "deepseek-reasoner"
      })
    );
  });

  it("未配置 general 策略显示缺口警告；保存策略按七 Role 提交", async () => {
    const w = await mountAt();
    await flushPromises();

    expect(
      w.find("[data-test='catalog-general-missing']").exists()
    ).toBe(true);
    await w.find("[data-test='catalog-policies-save']").trigger("click");
    await flushPromises();
    expect(setLlmTeamPolicies).toHaveBeenCalledTimes(1);
    const items = setLlmTeamPolicies.mock.calls[0][1] as {
      role_key: string;
      model_id: string | null;
    }[];
    expect(items).toHaveLength(7);
    expect(items[0]).toEqual({ role_key: "general", model_id: null });
  });
});
