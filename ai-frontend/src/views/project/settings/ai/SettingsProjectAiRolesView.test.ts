// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";

import SettingsProjectAiRolesView from "./SettingsProjectAiRolesView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { AiRoleVersion, ProjectAiRole } from "@/api/aiRoles";

const listProjectAiRoles = vi.fn();
const setProjectAiRoleEnabled = vi.fn();
const listProjectAiRoleVersions = vi.fn();

vi.mock("@/api/aiRoles", () => ({
  listProjectAiRoles: (...args: unknown[]) => listProjectAiRoles(...args),
  setProjectAiRoleEnabled: (...args: unknown[]) =>
    setProjectAiRoleEnabled(...args),
  listProjectAiRoleVersions: (...args: unknown[]) =>
    listProjectAiRoleVersions(...args)
}));

/** 后端真实行形状（ProjectAiRoleView 契约）的最小夹具 */
function role(overrides: Partial<ProjectAiRole>): ProjectAiRole {
  return {
    role_id: "role-1",
    role_key: "project_manager",
    name: "AI Project Manager",
    category: "management",
    enabled: false,
    current_version: null,
    ...overrides
  };
}

function version(overrides: Partial<AiRoleVersion>): AiRoleVersion {
  return {
    id: "ver-1",
    version: "1.0",
    created_by: null,
    created_at: "2026-11-05T00:00:00Z",
    definition: {},
    ...overrides
  };
}

const CATALOG: ProjectAiRole[] = [
  role({
    role_id: "r-pm",
    role_key: "project_manager",
    name: "AI Project Manager"
  }),
  role({
    role_id: "r-ra",
    role_key: "requirement_analyst",
    name: "Requirement Analyst",
    category: "product"
  }),
  role({
    role_id: "r-arch",
    role_key: "architect",
    name: "Architect",
    category: "architecture",
    enabled: true,
    current_version: {
      id: "v-arch",
      version: "1.2",
      created_at: "2026-11-06T00:00:00Z"
    }
  }),
  role({
    role_id: "r-dev",
    role_key: "developer",
    name: "Developer",
    category: "engineering"
  }),
  role({
    role_id: "r-qa",
    role_key: "tester",
    name: "Tester",
    category: "quality"
  })
];

async function mountView(path = "/project/settings/ai/roles?project_id=prj-1") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push(path);
  await router.isReady();
  const mounted = mountShell(SettingsProjectAiRolesView, {}, router);
  await flushPromises();
  return mounted;
}

function toggleOf(
  wrapper: ReturnType<typeof mountShell>["wrapper"],
  roleKey: string
) {
  return wrapper.find(
    `[data-testid="ai-role-toggle-${roleKey}"] [role="switch"]`
  );
}

describe("SettingsProjectAiRolesView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listProjectAiRoles.mockResolvedValue(CATALOG.map(row => ({ ...row })));
    listProjectAiRoleVersions.mockResolvedValue([]);
  });

  it("目录合并视图渲染五行：名称 / 类目徽标 / 当前版本 / 更新时间", async () => {
    const { wrapper } = await mountView();
    const list = wrapper.find('[data-testid="ai-roles-list"]');
    expect(list.exists()).toBe(true);
    expect(wrapper.findAll('[data-testid^="ai-role-row-"]')).toHaveLength(5);
    // 项目行缺失的模板行：enabled=false + 版本占位（懒生效口径）
    expect(list.text()).toContain("AI Project Manager");
    expect(list.text()).toContain("requirement_analyst");
    expect(list.text()).toContain("管理");
    expect(list.text()).toContain("工程");
    // 已启用行带版本摘要
    expect(list.text()).toContain("1.2");
  });

  it("无项目上下文时不发请求（scope 哨兵不得当作项目 id）", async () => {
    const { wrapper } = await mountView("/project/settings/ai/roles");
    expect(listProjectAiRoles).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="ai-roles-no-project"]').exists()).toBe(
      true
    );
  });

  it("启停成功：PUT 契约正确并刷新列表", async () => {
    setProjectAiRoleEnabled.mockResolvedValue(
      role({ role_id: "r-dev", role_key: "developer", enabled: true })
    );
    const { wrapper } = await mountView();
    expect(listProjectAiRoles).toHaveBeenCalledTimes(1);

    await toggleOf(wrapper, "developer").trigger("click");
    await flushPromises();

    expect(setProjectAiRoleEnabled).toHaveBeenCalledWith(
      "prj-1",
      "r-dev",
      true
    );
    // 操作后刷新列表（D-1：enable 懒生效，目录回读服务端真相）
    expect(listProjectAiRoles).toHaveBeenCalledTimes(2);
  });

  it("禁用 project_manager 返回 409：提示不可禁用且开关不翻转", async () => {
    setProjectAiRoleEnabled.mockRejectedValue(
      new HttpError("AI_ROLE_NOT_DISABLEABLE", { status: 409 })
    );
    const { wrapper } = await mountView();
    await toggleOf(wrapper, "project_manager").trigger("click");
    await flushPromises();

    const alert = wrapper.find('[data-testid="ai-roles-action-error"]');
    expect(alert.exists()).toBe(true);
    expect(alert.text()).toContain("AI Project Manager");
    expect(alert.text()).toContain("不可禁用");
    // 其他角色开关不受影响（409 只针对不可禁用角色）
    expect(
      toggleOf(wrapper, "developer").attributes("disabled")
    ).toBeUndefined();
  });

  it("403 无权限：全部开关进入禁用态并提示", async () => {
    setProjectAiRoleEnabled.mockRejectedValue(
      new HttpError("project manager required", { status: 403 })
    );
    const { wrapper } = await mountView();
    await toggleOf(wrapper, "developer").trigger("click");
    await flushPromises();

    const alert = wrapper.find('[data-testid="ai-roles-action-error"]');
    expect(alert.text()).toContain("无权限");
    for (const key of [
      "project_manager",
      "requirement_analyst",
      "architect",
      "developer",
      "tester"
    ]) {
      expect(toggleOf(wrapper, key).attributes("disabled")).toBeDefined();
    }
  });

  it("版本历史懒加载：首次展开才请求，收起再展开不重复请求", async () => {
    listProjectAiRoleVersions.mockResolvedValue([
      version({ id: "v1", version: "1.0", created_at: "2026-11-05T00:00:00Z" }),
      version({ id: "v2", version: "1.1", created_at: "2026-11-06T00:00:00Z" })
    ]);
    const { wrapper } = await mountView();
    expect(listProjectAiRoleVersions).not.toHaveBeenCalled();

    await wrapper
      .find('[data-testid="ai-role-history-architect"]')
      .trigger("click");
    await flushPromises();
    expect(listProjectAiRoleVersions).toHaveBeenCalledTimes(1);
    expect(listProjectAiRoleVersions).toHaveBeenCalledWith("prj-1", "r-arch");

    const panel = wrapper.find(
      '[data-testid="ai-role-history-panel-architect"]'
    );
    expect(panel.text()).toContain("1.0");
    expect(panel.text()).toContain("1.1");

    await wrapper
      .find('[data-testid="ai-role-history-architect"]')
      .trigger("click");
    await flushPromises();
    expect(
      wrapper.find('[data-testid="ai-role-history-panel-architect"]').exists()
    ).toBe(false);

    await wrapper
      .find('[data-testid="ai-role-history-architect"]')
      .trigger("click");
    await flushPromises();
    // 已缓存的展开不再重复请求
    expect(listProjectAiRoleVersions).toHaveBeenCalledTimes(1);
  });

  it("未启用角色展开历史显示空态文案（不渲染假版本）", async () => {
    listProjectAiRoleVersions.mockResolvedValue([]);
    const { wrapper } = await mountView();
    await wrapper
      .find('[data-testid="ai-role-history-developer"]')
      .trigger("click");
    await flushPromises();
    const panel = wrapper.find(
      '[data-testid="ai-role-history-panel-developer"]'
    );
    expect(panel.text()).toContain("尚无版本记录");
  });
});
