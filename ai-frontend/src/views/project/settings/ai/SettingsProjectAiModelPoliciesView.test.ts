// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";

import SettingsProjectAiModelPoliciesView from "./SettingsProjectAiModelPoliciesView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { LlmProvider, ProjectAiPolicy } from "@/api/llm";

const getProjectAiPolicy = vi.fn();
const setProjectAiPolicy = vi.fn();
const resolveTenantId = vi.fn();
const listLlmProviders = vi.fn();

vi.mock("@/api/llm", () => ({
  getProjectAiPolicy: (...args: unknown[]) => getProjectAiPolicy(...args),
  setProjectAiPolicy: (...args: unknown[]) => setProjectAiPolicy(...args),
  resolveTenantId: (...args: unknown[]) => resolveTenantId(...args),
  listLlmProviders: (...args: unknown[]) => listLlmProviders(...args)
}));

/** 后端真实行形状（ProjectAiPolicyRow 契约）的最小夹具 */
function policy(overrides: Partial<ProjectAiPolicy>): ProjectAiPolicy {
  return {
    project_id: "prj-1",
    provider_id: null,
    provider_name: null,
    provider_status: null,
    revision: 1,
    updated_at: null,
    ...overrides
  };
}

function provider(overrides: Partial<LlmProvider>): LlmProvider {
  return {
    id: "prov-1",
    name: "Team DeepSeek",
    provider_type: "deepseek",
    endpoint: "https://api.deepseek.com",
    configuration: { model: "deepseek-chat" },
    status: "enabled",
    timeout_secs: 120,
    revision: 2,
    has_credential: true,
    credential_hint: "****abcd",
    last_tested_at: null,
    created_by: "p-1",
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-02T00:00:00Z",
    ...overrides
  };
}

const ENABLED_PROVIDERS = [
  provider({ id: "prov-1", name: "Team DeepSeek" }),
  provider({ id: "prov-2", name: "Disabled One", status: "disabled" })
];

async function mountView(
  path = "/project/settings/ai/model-policies?project_id=prj-1"
) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push(path);
  await router.isReady();
  const mounted = mountShell(SettingsProjectAiModelPoliciesView, {}, router);
  await flushPromises();
  return mounted;
}

/** 打开 Select 浮层并选中指定文案的选项（reka-ui pointerdown/pointerup 交互） */
async function chooseOption(
  wrapper: ReturnType<typeof mountShell>["wrapper"],
  label: string
) {
  await wrapper.find('[role="combobox"]').trigger("pointerdown");
  await flushPortal();
  const option = Array.from(
    document.querySelectorAll<HTMLElement>('[role="option"]')
  ).find(node => node.textContent?.includes(label));
  expect(option, `option "${label}" should exist`).toBeTruthy();
  option!.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
  await flushPortal();
}

async function flushPortal() {
  await flushPromises();
  await new Promise(resolve => setTimeout(resolve, 0));
  await flushPromises();
}

describe("SettingsProjectAiModelPoliciesView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getProjectAiPolicy.mockResolvedValue(policy({}));
    resolveTenantId.mockResolvedValue("tenant-1");
    listLlmProviders.mockResolvedValue({
      items: ENABLED_PROVIDERS.map(row => ({ ...row })),
      total: 2
    });
  });

  it("无项目上下文时不发请求（scope 哨兵不得当作项目 id）", async () => {
    const { wrapper } = await mountView("/project/settings/ai/model-policies");
    expect(getProjectAiPolicy).not.toHaveBeenCalled();
    expect(resolveTenantId).not.toHaveBeenCalled();
    expect(
      wrapper.find('[data-testid="ai-policies-no-project"]').exists()
    ).toBe(true);
  });

  it("读：未指定 = 继承 Team 默认语义呈现；Provider 只列 enabled", async () => {
    const { wrapper } = await mountView();
    expect(wrapper.find('[data-testid="ai-policies-inherit"]').exists()).toBe(
      true
    );
    expect(
      wrapper.find('[data-testid="ai-policies-current"]').text()
    ).toContain("继承 Team 默认");
    expect(
      wrapper.find('[data-testid="ai-policies-current"]').text()
    ).toContain("首个已启用");
    // 下拉含继承哨兵 + enabled 两个实例；disabled 的 "Disabled One" 不进选项
    await wrapper.find('[role="combobox"]').trigger("pointerdown");
    await flushPortal();
    const labels = Array.from(document.querySelectorAll('[role="option"]')).map(
      node => node.textContent ?? ""
    );
    expect(labels).toHaveLength(2);
    expect(labels.join("\n")).toContain("Team DeepSeek");
    expect(labels.join("\n")).not.toContain("Disabled One");
    document.body.innerHTML = "";
  });

  it("读：已指定且 Provider 已停用 → 严格不回退警告", async () => {
    getProjectAiPolicy.mockResolvedValue(
      policy({
        provider_id: "prov-2",
        provider_name: "Disabled One",
        provider_status: "disabled",
        revision: 4,
        updated_at: "2026-11-07T00:00:00Z"
      })
    );
    const { wrapper } = await mountView();
    const current = wrapper.find('[data-testid="ai-policies-current"]');
    expect(current.text()).toContain("Disabled One");
    expect(current.text()).toContain("已停用");
    expect(current.text()).toContain("严格不回退");
  });

  it("存：选择 Provider 后 PUT 契约（provider_id + expected_revision），成功后按钮复位", async () => {
    setProjectAiPolicy.mockResolvedValue(
      policy({
        provider_id: "prov-1",
        provider_name: "Team DeepSeek",
        provider_status: "enabled",
        revision: 2,
        updated_at: "2026-11-08T00:00:00Z"
      })
    );
    const { wrapper } = await mountView();
    const save = () => wrapper.find('[data-testid="ai-policies-save"]');
    // 未改动时保存禁用（避免无意义 CAS 写）
    expect(save().attributes("disabled")).toBeDefined();

    await chooseOption(wrapper, "Team DeepSeek");
    expect(save().attributes("disabled")).toBeUndefined();
    await save().trigger("click");
    await flushPromises();

    expect(setProjectAiPolicy).toHaveBeenCalledTimes(1);
    expect(setProjectAiPolicy).toHaveBeenCalledWith("prj-1", {
      provider_id: "prov-1",
      expected_revision: 1
    });
    // 保存成功 → 应用响应行（revision=2），dirty 复位禁用
    expect(save().attributes("disabled")).toBeDefined();
    expect(
      wrapper.find('[data-testid="ai-policies-current"]').text()
    ).toContain("Team DeepSeek");
  });

  it("存：恢复继承 = PUT provider_id null", async () => {
    getProjectAiPolicy.mockResolvedValue(
      policy({
        provider_id: "prov-1",
        provider_name: "Team DeepSeek",
        provider_status: "enabled",
        revision: 2
      })
    );
    setProjectAiPolicy.mockResolvedValue(policy({ revision: 3 }));
    const { wrapper } = await mountView();

    await chooseOption(wrapper, "继承 Team 默认");
    await wrapper.find('[data-testid="ai-policies-save"]').trigger("click");
    await flushPromises();

    expect(setProjectAiPolicy).toHaveBeenCalledWith("prj-1", {
      provider_id: null,
      expected_revision: 2
    });
    expect(wrapper.find('[data-testid="ai-policies-inherit"]').exists()).toBe(
      true
    );
  });

  it("409 revision 冲突：提示重试并自动回读最新策略（CAS 可重试）", async () => {
    setProjectAiPolicy.mockRejectedValue(
      new HttpError("revision conflict: expected 1, actual 2", {
        code: "revision_conflict",
        status: 409
      })
    );
    const { wrapper } = await mountView();
    expect(getProjectAiPolicy).toHaveBeenCalledTimes(1);

    await chooseOption(wrapper, "Team DeepSeek");
    await wrapper.find('[data-testid="ai-policies-save"]').trigger("click");
    await flushPromises();

    const conflict = wrapper.find('[data-testid="ai-policies-conflict"]');
    expect(conflict.exists()).toBe(true);
    expect(conflict.text()).toContain("版本冲突");
    // 自动回读最新策略（第二次 GET），用户可基于新 revision 重试
    expect(getProjectAiPolicy).toHaveBeenCalledTimes(2);
    expect(wrapper.find('[data-testid="ai-policies-error"]').exists()).toBe(
      false
    );
  });

  it("团队无 enabled Provider：提示先去团队设置启用（不阻塞策略展示）", async () => {
    listLlmProviders.mockResolvedValue({ items: [], total: 0 });
    const { wrapper } = await mountView();
    expect(
      wrapper.find('[data-testid="ai-policies-no-providers"]').text()
    ).toContain("暂无已启用");
    expect(
      wrapper
        .find('[data-testid="ai-policies-no-providers"] a')
        .attributes("href")
    ).toBe("/settings/team/ai-providers");
  });
});
