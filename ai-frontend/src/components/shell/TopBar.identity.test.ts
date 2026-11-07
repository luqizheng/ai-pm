// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { DOMWrapper, flushPromises, type VueWrapper } from "@vue/test-utils";

/**
 * 顶栏账户菜单身份接线：身份块 = 当前登录用户（非演示数据）。
 *
 * auth store 在模块初始化时读取持久化用户，故「已登录」前置须在本文件
 * 整体以 getStoredUser 注入（与未登录用例的 TopBar.test.ts 分文件）。
 */
const { storedUser, getMe, fetchTeamContext } = vi.hoisted(() => ({
  storedUser: {
    id: "u-1",
    username: "admin",
    nickname: "管理员",
    email: "admin@aipm.dev",
    phone: null,
    avatar: null,
    roles: [],
    permissions: []
  },
  getMe: vi.fn(),
  fetchTeamContext: vi.fn()
}));

vi.mock("@/api/base/token-storage", async importOriginal => ({
  ...(await importOriginal<typeof import("@/api/base/token-storage")>()),
  getStoredUser: () => storedUser
}));

vi.mock("@/api/base/self-service", () => ({
  getMe: (...args: unknown[]) => getMe(...args),
  resolveAvatarSrc: vi.fn(async () => undefined)
}));

vi.mock("@/api/team", () => ({
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

import TopBar from "./TopBar.vue";
import { mountShell } from "./test-utils";

const TEAM_CONTEXT = {
  principal_id: "p-1",
  display_name: "admin",
  is_active: true,
  is_admin: true,
  tenant_id: "t-1",
  tenant_role: "owner",
  team: { id: "t-1", name: "Acme R&D", slug: "acme-rd", description: null },
  onboarding_required: false,
  memberships: [
    { team_id: "t-1", team_name: "Acme R&D", role: "owner", status: "active" }
  ]
};

const ME = {
  ...storedUser,
  status: "active",
  email_verified: true,
  phone_verified: false,
  created_at: "2026-08-11T00:00:00Z",
  updated_at: "2026-11-07T00:00:00Z"
};

let wrapper: VueWrapper | null = null;

async function openMenu() {
  wrapper = mountShell(TopBar, { attachTo: document.body }).wrapper;
  await flushPromises();
  await wrapper.find(".user-menu-button").trigger("click");
  await nextTick();
  return document.body.querySelector<HTMLElement>(".user-menu");
}

describe("TopBar 账户菜单身份接线", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getMe.mockResolvedValue({ ...ME });
    fetchTeamContext.mockResolvedValue(TEAM_CONTEXT);
  });

  afterEach(() => {
    wrapper?.unmount();
    wrapper = null;
    document.body.innerHTML = "";
  });

  it("身份块显示登录用户（昵称/邮箱），不再是演示数据", async () => {
    const panel = await openMenu();

    expect(panel?.querySelector(".user-menu__name")?.textContent).toBe(
      "管理员"
    );
    expect(panel?.querySelector(".user-menu__email")?.textContent?.trim()).toBe(
      "admin@aipm.dev"
    );
    expect(panel?.textContent).not.toContain("Alex Morgan");
  });

  it("挂载即以 /me 校正资料（别处改过昵称后顶栏随之更新）", async () => {
    getMe.mockResolvedValue({ ...ME, nickname: "管理员（新）" });

    const panel = await openMenu();

    expect(getMe).toHaveBeenCalledTimes(1);
    expect(panel?.querySelector(".user-menu__name")?.textContent).toBe(
      "管理员（新）"
    );
  });

  it("组织行 = 当前团队（刷新页面后挂载即补拉团队上下文），无项目角色行", async () => {
    const panel = await openMenu();

    expect(fetchTeamContext).toHaveBeenCalledTimes(1);
    const context = Array.from(
      panel?.querySelectorAll(".user-menu__context") ?? []
    ).map(el => el.textContent?.trim());
    expect(context).toEqual(["组织 · Acme R&D"]);
    expect(wrapper!.find(".team-switcher").text()).toContain("Acme R&D");
  });

  it("退出确认弹窗回显登录用户邮箱", async () => {
    const panel = await openMenu();

    const signOut = Array.from(
      panel?.querySelectorAll<HTMLButtonElement>(".user-menu-item") ?? []
    ).find(b => b.textContent?.includes("退出登录"));
    await new DOMWrapper(signOut!).trigger("click");
    await flushPromises();

    expect(
      document.body.querySelector("#user-signout-desc")?.textContent
    ).toContain("admin@aipm.dev");
  });
});
