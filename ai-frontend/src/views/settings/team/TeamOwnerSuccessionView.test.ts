// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell, settle, waitForRoute } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import TeamOwnerSuccessionView from "./TeamOwnerSuccessionView.vue";

/**
 * Team Owner 继任页契约（P4-ORG UX 修订 V1.0 / Figma T3）：
 *   - 三步流程指示 + 继任者必选（合格 = 自己之外的已激活成员）；
 *   - 影响复核随继任者变化（现任 Owner 继任 → 保持；成员继任 → Owner 数补入）；
 *   - 确认 = 先任命继任者为 owner、再把自己降为 member（顺序保证零 Owner
 *     态不可能出现）；完成后带保存态 query 回成员页；
 *   - 末位 Owner 保护警告常驻；非 Team Owner 直接回成员页。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";

const mountedWrappers: VueWrapper[] = [];

const listTeamMembers = vi.fn();
const setTeamMemberRole = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/team", () => ({
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  setTeamMemberRole: (...args: unknown[]) => setTeamMemberRole(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

function member(overrides: Record<string, unknown> = {}) {
  return {
    principal_id: "p-bob",
    display_name: "bob",
    role: "member",
    is_active: true,
    created_at: "2026-11-02T09:30:00Z",
    ...overrides
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/settings/team/members/owner-succession");
  await router.isReady();
  // attachTo：Select 的键盘交互依赖 document.activeElement。
  const wrapper = mountShell(TeamOwnerSuccessionView, {
    global: { plugins: [router] },
    attachTo: document.body
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  return { wrapper, router };
}

function contextFor(principalId: string, role: "owner" | "member" = "owner") {
  return {
    principal_id: principalId,
    display_name: "Alex Morgan",
    is_active: true,
    is_admin: false,
    tenant_id: "00000000-0000-0000-0000-000000000099",
    team_role: role,
    team: {
      id: TEAM_ID,
      name: "Platform Engineering",
      slug: "PE",
      description: null
    },
    onboarding_required: false,
    memberships: [
      {
        team_id: TEAM_ID,
        team_name: "Platform Engineering",
        role,
        status: "active"
      }
    ]
  };
}

function pressKey(key: string): void {
  document.activeElement?.dispatchEvent(
    new KeyboardEvent("keydown", { key, bubbles: true })
  );
}

/** 打开 Select 并选中第 n 个合格继任者（0 起；SelectItem 自带 keydown 选择处理）。 */
async function chooseSuccessor(
  wrapper: VueWrapper,
  index: number
): Promise<void> {
  const trigger = wrapper.find<HTMLElement>('[role="combobox"]');
  trigger.element.focus();
  pressKey("ArrowDown");
  await settle();
  // 方向键逐项下移高亮，目标项上直接派发 Enter（不依赖浮层焦点模拟）。
  for (let i = 0; i < index; i += 1) {
    pressKey("ArrowDown");
    await settle();
  }
  const options = Array.from(
    document.querySelectorAll<HTMLElement>('[role="option"]')
  );
  const target = options[index];
  if (!target) throw new Error(`successor option ${index} not found`);
  target.dispatchEvent(
    new KeyboardEvent("keydown", {
      key: "Enter",
      bubbles: true,
      cancelable: true
    })
  );
  await settle();
}

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

beforeEach(() => {
  vi.clearAllMocks();
  // 断言按设计稿英文文案（i18n 默认随 localStorage 落 zh-CN）。
  i18n.global.locale.value = "en";
  listTeamMembers.mockResolvedValue([
    member({
      principal_id: "p-alex",
      display_name: "Alex Morgan",
      role: "owner"
    }),
    member({
      principal_id: "p-maya",
      display_name: "Maya Chen",
      role: "owner"
    }),
    member({ principal_id: "p-jordan", display_name: "Jordan Lee" })
  ]);
  fetchTeamContext.mockResolvedValue(contextFor("p-alex"));
});

describe("TeamOwnerSuccessionView", () => {
  it("shows the stepper, last-owner warning and successor-dependent review", async () => {
    const { wrapper } = await mountAt();
    // 三步指示 + 末位 Owner 保护警告常驻。
    expect(wrapper.find("[data-testid='succession-steps']").exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("Last Owner protection");
    // 标题 = 当前 Owner → 未选继任者；确认按钮禁用。
    expect(wrapper.find("[data-testid='succession-heading']").text()).toBe(
      "Alex Morgan → —"
    );
    expect(
      (
        wrapper.find("[data-test='succession-confirm']")
          .element as HTMLButtonElement
      ).disabled
    ).toBe(true);

    // 选现任 Owner（Maya，首个合格项）→ 保持 Owner；自己降 Member；团队保留 1 名。
    await chooseSuccessor(wrapper, 0);
    expect(wrapper.find("[data-testid='succession-heading']").text()).toBe(
      "Alex Morgan → Maya Chen"
    );
    expect(
      wrapper.find("[data-testid='succession-impact-successor']").text()
    ).toContain("Maya Chen remains Team Owner.");
    expect(
      wrapper.find("[data-testid='succession-impact-successor']").text()
    ).toContain("Alex Morgan becomes Team Member.");
    expect(
      wrapper.find("[data-testid='succession-impact-retain']").text()
    ).toContain("Team retains 1 Owner;");
    expect(
      (
        wrapper.find("[data-test='succession-confirm']")
          .element as HTMLButtonElement
      ).disabled
    ).toBe(false);
  });

  it("reviews member succession as a new owner with the owner count restored", async () => {
    const { wrapper } = await mountAt();
    // 成员继任（Jordan，第二合格项）→ 成为 Owner，Owner 数补入（2 - 1 + 1 = 2）。
    await chooseSuccessor(wrapper, 1);
    expect(wrapper.find("[data-testid='succession-heading']").text()).toBe(
      "Alex Morgan → Jordan Lee"
    );
    expect(
      wrapper.find("[data-testid='succession-impact-successor']").text()
    ).toContain("Jordan Lee becomes Team Owner.");
    expect(
      wrapper.find("[data-testid='succession-impact-retain']").text()
    ).toContain("Team retains 2 Owner;");
  });

  it("appoints the successor first, then demotes the caller, then redirects with the saved state", async () => {
    setTeamMemberRole.mockResolvedValue(undefined);
    const { wrapper, router } = await mountAt();
    await chooseSuccessor(wrapper, 0);
    await wrapper.find("[data-test='succession-confirm']").trigger("click");
    await settle();
    // 顺序 = 先任命（零 Owner 态不可能出现）后降级。
    expect(setTeamMemberRole.mock.calls).toEqual([
      [TEAM_ID, "p-maya", "owner"],
      [TEAM_ID, "p-alex", "member"]
    ]);
    await waitForRoute(router, "/settings/team/members");
    expect(router.currentRoute.value.path).toBe("/settings/team/members");
    expect(router.currentRoute.value.query.owner_changed).toBe("1");
    expect(router.currentRoute.value.query.owner_changed_to).toBe("Maya Chen");
  });

  it("redirects plain members back to the members page", async () => {
    fetchTeamContext.mockResolvedValue(contextFor("p-jordan", "member"));
    const { router } = await mountAt();
    await settle();
    await waitForRoute(router, "/settings/team/members");
    expect(router.currentRoute.value.path).toBe("/settings/team/members");
  });
});
