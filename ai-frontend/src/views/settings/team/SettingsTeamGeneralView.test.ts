// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { DOMWrapper, flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import SettingsTeamGeneralView from "./SettingsTeamGeneralView.vue";

/**
 * 团队设置 — 基本页契约：
 *   - Danger Zone（解散团队）仅 Owner 可见（产品裁决 Owner 独占）；
 *   - 输入团队名一致才解锁解散按钮；确认后调 dissolve 端点并登出回登录页；
 *   - 非 Owner 视角不渲染危险区与转移卡（细则 #3 角色即入口）。
 */
const TENANT_ID = "00000000-0000-0000-0000-000000000010";

const mountedWrappers: VueWrapper[] = [];

const fetchTeamProfile = vi.fn();
const listTeamMembers = vi.fn();
const dissolveTeam = vi.fn();
const updateTeamProfile = vi.fn();
const transferTeamOwnership = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/team", () => ({
  fetchTeamProfile: (...args: unknown[]) => fetchTeamProfile(...args),
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  dissolveTeam: (...args: unknown[]) => dissolveTeam(...args),
  updateTeamProfile: (...args: unknown[]) => updateTeamProfile(...args),
  transferTeamOwnership: (...args: unknown[]) => transferTeamOwnership(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

const signOut = vi.fn();

vi.mock("@/stores/auth", () => ({
  useAuthStore: () => ({ signOut })
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: () => {} })
}));

function context(role: string) {
  return {
    principal_id: role === "owner" ? "p-owner" : "p-carol",
    display_name: role,
    is_active: true,
    is_admin: role !== "member",
    tenant_id: TENANT_ID,
    tenant_role: role,
    team: {
      id: TENANT_ID,
      name: "Acme R&D",
      slug: "acme-rd",
      description: null
    },
    onboarding_required: false,
    memberships: [
      { team_id: TENANT_ID, team_name: "Acme R&D", role, status: "active" }
    ]
  };
}

async function mountAt(path = "/settings/team/general") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(SettingsTeamGeneralView, {
    global: { plugins: [router] }
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  return { wrapper, router };
}

/** Danger Zone 输入的原生 input（Input 组件根是 DIV，id 显式落在 native 上） */
function dissolveInput(wrapper: VueWrapper) {
  return wrapper.find("input#dissolve-confirm");
}

function dissolveButton(wrapper: VueWrapper) {
  return wrapper.find("[data-test='team-dissolve-open']")
    .element as HTMLButtonElement;
}

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

beforeEach(() => {
  vi.clearAllMocks();
  fetchTeamContext.mockResolvedValue(context("owner"));
  fetchTeamProfile.mockResolvedValue({
    id: TENANT_ID,
    name: "Acme R&D",
    slug: "acme-rd",
    description: null
  });
  listTeamMembers.mockResolvedValue([
    {
      principal_id: "p-owner",
      display_name: "admin",
      role: "owner",
      is_active: true,
      created_at: "x"
    }
  ]);
});

describe("SettingsTeamGeneralView", () => {
  it("as OWNER shows the danger zone; name must match to unlock dissolve", async () => {
    const { wrapper } = await mountAt();
    expect(dissolveButton(wrapper).disabled).toBe(true);

    await dissolveInput(wrapper).setValue("Wrong Name");
    expect(dissolveButton(wrapper).disabled).toBe(true);

    await dissolveInput(wrapper).setValue("Acme R&D");
    expect(dissolveButton(wrapper).disabled).toBe(false);
  });

  it("as OWNER dissolving calls the endpoint and signs out to login", async () => {
    const { wrapper, router } = await mountAt();
    dissolveTeam.mockResolvedValue({ projects_deleted: 2, members_removed: 3 });

    await dissolveInput(wrapper).setValue("Acme R&D");
    await wrapper.find("[data-test='team-dissolve-open']").trigger("click");
    await flushPromises();
    const pushSpy = vi.spyOn(router, "push");
    // Dialog 经 reka Portal 挂到 document.body，wrapper.find 不可达。
    const confirmEl = document.body.querySelector(
      "[data-test='team-dissolve-confirm-button']"
    ) as HTMLButtonElement | null;
    expect(confirmEl).not.toBeNull();
    await new DOMWrapper(confirmEl).trigger("click");
    await flushPromises();
    await flushPromises();

    expect(dissolveTeam).toHaveBeenCalledWith(TENANT_ID);
    expect(signOut).toHaveBeenCalled();
    // 断言「登出 + 发起回登录页导航」的契约本身；整页落定依赖 LoginView
    // 懒加载链（happy-dom 下可能中断，属集成范畴）。
    expect(pushSpy).toHaveBeenCalledWith("/auth/login");
  });

  it("as ADMIN the danger zone and ownership transfer are not rendered", async () => {
    fetchTeamContext.mockResolvedValue(context("admin"));
    const { wrapper } = await mountAt();
    expect(wrapper.find("[data-test='team-dissolve-open']").exists()).toBe(
      false
    );
    expect(wrapper.find("[data-test='team-transfer-open']").exists()).toBe(
      false
    );
  });
});
