// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectMembersView from "./SettingsProjectMembersView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

/**
 * P1 Project Settings · Members 页面契约（全部走既有授权面）：
 *   - 列表呈现角色徽标（owner 无移除/转移动作）；
 *   - 添加 = POST members（确认对话框 + 主体候选）；
 *   - 移除 / 所有权转移必须经确认对话框。
 */
const listProjectMembers = vi.fn();
const grantProjectMember = vi.fn();
const revokeProjectMember = vi.fn();
const transferProjectOwnership = vi.fn();
const listPrincipals = vi.fn();

vi.mock("@/api/projects", () => ({
  getProjectInfo: vi.fn().mockResolvedValue({}),
  updateProjectInfo: vi.fn(),
  closeProject: vi.fn(),
  reopenProject: vi.fn(),
  listProjectMembers: (...args: unknown[]) => listProjectMembers(...args),
  grantProjectMember: (...args: unknown[]) => grantProjectMember(...args),
  revokeProjectMember: (...args: unknown[]) => revokeProjectMember(...args),
  transferProjectOwnership: (...args: unknown[]) =>
    transferProjectOwnership(...args),
  listPrincipals: (...args: unknown[]) => listPrincipals(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function memberFixture(overrides: Record<string, unknown> = {}) {
  return {
    principal_id: "11111111-1111-1111-1111-111111111111",
    display_name: "张三",
    role: "member",
    kind: "human",
    is_active: true,
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push("/project/settings/members?project_id=prj-1");
  await router.isReady();
  const mounted = mountShell(SettingsProjectMembersView, {}, router);
  const wrapper = mounted.wrapper;
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  vi.clearAllMocks();
  listProjectMembers.mockResolvedValue({
    items: [
      memberFixture(),
      memberFixture({
        principal_id: "22222222-2222-2222-2222-222222222222",
        display_name: "李四",
        role: "owner"
      })
    ]
  });
  listPrincipals.mockResolvedValue({
    items: [
      {
        id: "33333333-3333-3333-3333-333333333333",
        display_name: "王五",
        kind: "human",
        is_active: true
      }
    ],
    total: 1
  });
  grantProjectMember.mockResolvedValue({ principal_id: "3", role: "member" });
});

describe("SettingsProjectMembersView", () => {
  it("renders member rows with roles; owner row has no remove/transfer actions", async () => {
    const wrapper = await mountView();
    const rows = wrapper.findAll("[data-testid='pm-row']");
    expect(rows.length).toBe(2);
    expect(wrapper.text()).toContain("张三");
    expect(wrapper.text()).toContain("李四");
    expect(wrapper.find("[data-testid='pm-remove-22222222']").exists()).toBe(
      false
    );
    expect(wrapper.find("[data-testid='pm-remove-11111111']").exists()).toBe(
      true
    );
  });

  it("grants membership via the inline add form", async () => {
    const wrapper = await mountView();
    await wrapper.find("[data-testid='pm-add-toggle']").trigger("click");
    await flushPromises();
    const select = wrapper.find("[data-testid='pm-candidate']");
    expect(select.exists()).toBe(true);
    await select.setValue("33333333-3333-3333-3333-333333333333");
    expect(
      wrapper.find("[data-testid='pm-grant']").attributes("disabled")
    ).toBeUndefined();
    await wrapper.find("[data-testid='pm-grant']").trigger("click");
    await flushPromises();
    expect(grantProjectMember).toHaveBeenCalledWith(
      "prj-1",
      "33333333-3333-3333-3333-333333333333"
    );
  });

  it("removes a member only after confirmation", async () => {
    const wrapper = await mountView();
    await wrapper.find("[data-testid='pm-remove-11111111']").trigger("click");
    await flushPromises();
    expect(revokeProjectMember).not.toHaveBeenCalled();
    const confirm = document.querySelector(
      ".ui-confirm__btn--danger:not([disabled])"
    ) as HTMLButtonElement | null;
    expect(confirm).not.toBeNull();
    confirm!.click();
    await flushPromises();
    expect(revokeProjectMember).toHaveBeenCalledWith(
      "prj-1",
      "11111111-1111-1111-1111-111111111111"
    );
  });
});
