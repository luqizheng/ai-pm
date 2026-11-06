// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import TeamInvitationAcceptView from "./TeamInvitationAcceptView.vue";

/**
 * Screens 06+07 — 接受页契约：预览卡展示团队/邀请人/角色；接受成功呈现
 * 结果面并给「进入团队主页」；未登录先跳登录（带 redirect 回跳）。
 */
const mountedWrappers: VueWrapper[] = [];

const previewInvitation = vi.fn();
const acceptTeamInvitation = vi.fn();
const previewInvitationLink = vi.fn();
const acceptInvitationLink = vi.fn();

vi.mock("@/api/team", () => ({
  previewInvitation: (...args: unknown[]) => previewInvitation(...args),
  acceptTeamInvitation: (...args: unknown[]) => acceptTeamInvitation(...args),
  previewInvitationLink: (...args: unknown[]) => previewInvitationLink(...args),
  acceptInvitationLink: (...args: unknown[]) => acceptInvitationLink(...args)
}));

// 工厂必须自包含：stores/auth 在模块初始化期就会调 getStoredUser（全局
// account 缓存），引用测试体 const 会必然 TDZ；句柄经动态导入获取。
vi.mock("@/api/base/token-storage", () => ({
  getAccessToken: vi.fn(() => "token"),
  getStoredUser: vi.fn(() => ({ email: "dave@example.com" }))
}));

async function tokenStorage() {
  return await import("@/api/base/token-storage");
}

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

async function mountAt(path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(TeamInvitationAcceptView, {
    global: { plugins: [router] }
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  await flushPromises();
  return { wrapper, router };
}

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

beforeEach(async () => {
  vi.clearAllMocks();
  const tokens = await tokenStorage();
  vi.mocked(tokens.getAccessToken).mockReturnValue("token");
  vi.mocked(tokens.getStoredUser).mockReturnValue({
    email: "dave@example.com"
  } as never);
  previewInvitation.mockResolvedValue({
    team_name: "Acme R&D",
    team_slug: "acme-rd",
    inviter_name: "admin",
    role: "member",
    expires_at: "2026-11-13T08:00:00Z",
    status: "PENDING",
    email: "dave@example.com"
  });
  acceptTeamInvitation.mockResolvedValue({
    outcome: "joined",
    team_id: "t-1",
    team_name: "Acme R&D",
    role: "member"
  });
});

describe("TeamInvitationAcceptView", () => {
  it("redirects to login with redirect when unauthenticated", async () => {
    const tokens = await tokenStorage();
    vi.mocked(tokens.getAccessToken).mockReturnValue(undefined);
    // spy 必须在挂载前装好：视图在 onMounted 里就发起跳转。
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push("/invitation/accept?token=tok&type=invite");
    await router.isReady();
    const replaceSpy = vi.spyOn(router, "replace");
    const wrapper = mountShell(TeamInvitationAcceptView, {
      global: { plugins: [router] }
    });
    mountedWrappers.push(wrapper);
    await flushPromises();
    expect(replaceSpy).toHaveBeenCalled();
    const target = replaceSpy.mock.calls[0]?.[0] as {
      path?: string;
      query?: Record<string, unknown>;
    };
    expect(target.path).toBe("/auth/login");
    expect(String(target.query?.redirect)).toContain("/invitation/accept");
  });

  it("renders the invitation preview card", async () => {
    const { wrapper } = await mountAt(
      "/invitation/accept?token=tok&type=invite"
    );
    expect(previewInvitation).toHaveBeenCalledWith("tok");
    expect(wrapper.text()).toContain("Acme R&D");
    expect(wrapper.text()).toContain("MEMBER");
    expect(wrapper.find("[data-testid='accept-confirm']").exists()).toBe(true);
  });

  it("shows the success result and dashboard action after accepting", async () => {
    const { wrapper } = await mountAt(
      "/invitation/accept?token=tok&type=invite"
    );
    await wrapper.find("[data-testid='accept-confirm']").trigger("click");
    await flushPromises();
    expect(acceptTeamInvitation).toHaveBeenCalledWith({
      token: "tok",
      email: "dave@example.com"
    });
    const result = wrapper.find("[data-testid='accept-result']");
    expect(result.exists()).toBe(true);
    expect(wrapper.find("[data-testid='accept-continue']").exists()).toBe(true);
  });

  it("maps backend stable errors to result states", async () => {
    previewInvitation.mockResolvedValue({
      team_name: "Acme R&D",
      team_slug: null,
      inviter_name: "admin",
      role: "member",
      expires_at: "2026-11-13T08:00:00Z",
      status: "EXPIRED",
      email: "dave@example.com"
    });
    const { wrapper } = await mountAt(
      "/invitation/accept?token=tok&type=invite"
    );
    expect(
      wrapper.find("[data-testid='accept-preview-blocked']").exists()
    ).toBe(true);
  });
});
