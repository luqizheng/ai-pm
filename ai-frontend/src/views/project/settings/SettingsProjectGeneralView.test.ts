// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectGeneralView from "./SettingsProjectGeneralView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

/**
 * P1 Project Settings · General 页面契约：
 *   - 基本信息：名称/描述编辑 → PATCH（守卫=生命周期管理者，403 走错误
 *     toast 不吞错）；code 只读展示；
 *   - 危险操作：关闭项目必须经 ConfirmationDialog（POST /close）；
 *     closed 态提供重开（POST /reopen）。
 */
const getProjectInfo = vi.fn();
const updateProjectInfo = vi.fn();
const closeProject = vi.fn();
const reopenProject = vi.fn();

vi.mock("@/api/projects", () => ({
  getProjectInfo: (...args: unknown[]) => getProjectInfo(...args),
  updateProjectInfo: (...args: unknown[]) => updateProjectInfo(...args),
  closeProject: (...args: unknown[]) => closeProject(...args),
  reopenProject: (...args: unknown[]) => reopenProject(...args),
  listProjectMembers: vi.fn().mockResolvedValue({ items: [] }),
  grantProjectMember: vi.fn(),
  revokeProjectMember: vi.fn(),
  transferProjectOwnership: vi.fn(),
  listPrincipals: vi.fn().mockResolvedValue({ items: [], total: 0 })
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function infoFixture(overrides: Record<string, unknown> = {}) {
  return {
    id: "prj-1",
    customer_id: "c-1",
    customer_name: "Acme",
    code: "PI-DEMO",
    name: "演示项目",
    description: "描述",
    status: "active",
    revision: 1,
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-01T00:00:00Z",
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push("/project/settings/general?project_id=prj-1");
  await router.isReady();
  const mounted = mountShell(SettingsProjectGeneralView, {}, router);
  const wrapper = mounted.wrapper;
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  vi.clearAllMocks();
  getProjectInfo.mockResolvedValue(infoFixture());
  updateProjectInfo.mockResolvedValue(infoFixture({ name: "新名字" }));
});

describe("SettingsProjectGeneralView", () => {
  it("renders read-only code and editable name; save disabled without changes", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("PI-DEMO");
    const name = wrapper.find("[data-testid='pg-name']");
    expect((name.element as HTMLInputElement).value).toBe("演示项目");
    expect(
      wrapper.find("[data-testid='pg-save']").attributes("disabled")
    ).toBeDefined();
  });

  it("saves edited info via updateProjectInfo", async () => {
    const wrapper = await mountView();
    await wrapper.find("[data-testid='pg-name']").setValue("新名字");
    const save = wrapper.find("[data-testid='pg-save']");
    expect(save.attributes("disabled")).toBeUndefined();
    await save.trigger("click");
    await flushPromises();
    expect(updateProjectInfo).toHaveBeenCalledWith("prj-1", {
      name: "新名字",
      description: "描述"
    });
  });

  it("requires confirmation before closing and calls closeProject", async () => {
    const wrapper = await mountView();
    await wrapper.find("[data-testid='pg-close']").trigger("click");
    await flushPromises();
    expect(closeProject).not.toHaveBeenCalled();
    // ConfirmationDialog Teleport 到 body：按 footer 结构定位确认按钮
    //（.ui-confirm__btn--danger = 关闭项目的 danger 确认）。
    const confirm = document.querySelector(
      ".ui-confirm__btn--danger:not([disabled])"
    ) as HTMLButtonElement | null;
    expect(confirm).not.toBeNull();
    confirm!.click();
    await flushPromises();
    expect(closeProject).toHaveBeenCalledWith("prj-1", undefined);
  });

  it("shows reopen action when the project is closed", async () => {
    getProjectInfo.mockResolvedValue(infoFixture({ status: "closed" }));
    const wrapper = await mountView();
    expect(wrapper.find("[data-testid='pg-close']").exists()).toBe(false);
    expect(wrapper.find("[data-testid='pg-reopen']").exists()).toBe(true);
  });
});
