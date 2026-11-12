// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

const listProjectAiRoles = vi.fn();
const getProjectAiPolicy = vi.fn();
const resolveTenantId = vi.fn();
const listLlmProviders = vi.fn();
const getProjectAiTeamMonitor = vi.fn();
const getProjectAiExecutionPolicies = vi.fn();

vi.mock("@/api/aiRoles", () => ({
  listProjectAiRoles: (...args: unknown[]) => listProjectAiRoles(...args),
  setProjectAiRoleEnabled: vi.fn(),
  listProjectAiRoleVersions: vi.fn()
}));
vi.mock("@/api/llm", () => ({
  getProjectAiPolicy: (...args: unknown[]) => getProjectAiPolicy(...args),
  setProjectAiPolicy: vi.fn(),
  resolveTenantId: (...args: unknown[]) => resolveTenantId(...args),
  listLlmProviders: (...args: unknown[]) => listLlmProviders(...args)
}));
vi.mock("@/api/aiTeam", () => ({
  getProjectAiTeamOverview: vi.fn(),
  getProjectAiTeamMonitor: (...args: unknown[]) =>
    getProjectAiTeamMonitor(...args),
  getProjectAiExecutionPolicies: (...args: unknown[]) =>
    getProjectAiExecutionPolicies(...args),
  setProjectExecutionPolicy: vi.fn(),
  setTaskPolicyOverride: vi.fn(),
  clearTaskPolicyOverride: vi.fn()
}));

/**
 * AI Team 骨架（UI-1）：五子页子路由 + 页内页签条；入口重定向到 Roles。
 * 页面经路由表渲染（含 AiTeamFrame 的设置域侧栏），断言骨架契约本身。
 */
async function navigate(path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push(path);
  await router.isReady();
  const view = router.currentRoute.value.matched[0]?.components?.default;
  const mounted = mountShell(view as never, {}, router);
  await flushPromises();
  return { router, ...mounted };
}

describe("Project Setting · AI Team 骨架", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // 内容区数据由各自视图测试覆盖；这里只喂最小读模型
    listProjectAiRoles.mockResolvedValue([]);
    getProjectAiPolicy.mockResolvedValue({
      project_id: "prj-1",
      provider_id: null,
      provider_name: null,
      provider_status: null,
      revision: 1,
      updated_at: null
    });
    resolveTenantId.mockResolvedValue("tenant-1");
    listLlmProviders.mockResolvedValue({ items: [], total: 0 });
  });

  it("/project/settings/ai 重定向到 Roles 子页（落地序第一）", async () => {
    const { router } = await navigate("/project/settings/ai?project_id=prj-1");
    expect(router.currentRoute.value.path).toBe("/project/settings/ai/roles");
  });

  it("页签条渲染五子页并携带项目上下文，当前子页高亮", async () => {
    const { wrapper } = await navigate(
      "/project/settings/ai/roles?project_id=prj-1"
    );
    const tabs = wrapper.findAll('[data-testid="ai-tabs"] a');
    expect(tabs).toHaveLength(5);
    expect(tabs[0].text()).toBe("角色");
    expect(tabs[0].classes()).toContain("is-active");
    expect(tabs[1].attributes("href")).toBe(
      "/project/settings/ai/overview?project_id=prj-1"
    );
    expect(tabs[2].text()).toBe("模型策略");
    expect(tabs[3].text()).toBe("执行监控");
    expect(tabs[4].text()).toBe("执行策略");
  });

  it("Execution Policies 子页落地后渲染内容区（五子页无占位残留），并保留页签条", async () => {
    // UI-3 起 Execution Policies 有了真实内容区（分层卡 + 合成规则），
    // 「即将提供」占位视图与 aiTeam.comingSoon 文案已一并移除。
    getProjectAiExecutionPolicies.mockResolvedValue({
      organization: null,
      project: null,
      taskTypes: [],
      taskOverrides: [],
      composition: {
        layer_order: ["organization", "project", "task_type", "task"],
        defaults: {
          auto_accept: true,
          auto_merge: true,
          auto_publish_knowledge: true,
          require_human_approval: false,
          additional_checks: []
        },
        rules: []
      }
    });
    const { wrapper } = await navigate(
      "/project/settings/ai/execution-policies?project_id=prj-1"
    );
    expect(getProjectAiExecutionPolicies).toHaveBeenCalledWith("prj-1");
    expect(
      wrapper.find('[data-testid="ai-exec-pol-composition"]').exists()
    ).toBe(true);
    expect(wrapper.find('[data-testid="ai-coming-soon"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="ai-tabs"]').exists()).toBe(true);
  });

  it("侧栏「AI 配置」条目保持激活且带项目上下文（设置域导航同构）", async () => {
    const { wrapper } = await navigate(
      "/project/settings/ai/model-policies?project_id=prj-1"
    );
    const active = wrapper.find(".settings-nav-item.is-active");
    expect(active.attributes("href")).toBe(
      "/project/settings/ai?project_id=prj-1"
    );
    const tabs = wrapper.findAll('[data-testid="ai-tabs"] a');
    const modelPoliciesTab = tabs.find(a => a.text().includes("模型策略"));
    expect(modelPoliciesTab?.attributes("href")).toBe(
      "/project/settings/ai/model-policies?project_id=prj-1"
    );
    expect(modelPoliciesTab?.classes()).toContain("is-active");
  });
});
