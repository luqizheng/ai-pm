// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

const { authGateway } = vi.hoisted(() => ({
  authGateway: {
    signIn: vi.fn(),
    signOut: vi.fn(async () => {}),
    requestPasswordReset: vi.fn(async () => {}),
    resetPassword: vi.fn(async () => {}),
    acceptInvitation: vi.fn()
  }
}));

vi.mock("@/api/auth", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/auth")>();
  return { ...actual, authGateway };
});

import InvitationView from "./InvitationView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import { waitForRoute } from "@/lib/test-utils";
import { AUTH_PATHS } from "@/router/auth";

const INVITATION_SESSION = {
  account: "alex.morgan@acme.dev",
  organization: "Acme Research",
  user: {
    id: "u-2",
    username: "alex.morgan@acme.dev",
    nickname: "Alex Morgan",
    email: "alex.morgan@acme.dev",
    phone: null,
    avatar: null,
    roles: ["member"],
    permissions: []
  }
};

/**
 * 邀请接受页行为：无票据失效态、带票据设置口令后进入认证进度页、换账号入口。
 */
describe("InvitationView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("缺少邀请票据时展示失效态", async () => {
    const { wrapper, router } = mountShell(InvitationView);
    await router.isReady();
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.invitation.errorTitle")
    );
  });

  it("设置口令并接受邀请后进入认证进度页", async () => {
    authGateway.acceptInvitation.mockResolvedValueOnce(INVITATION_SESSION);
    const { wrapper, router } = mountShell(InvitationView);
    await router.push(`${AUTH_PATHS.invitation}?token=tok-1`);
    await router.isReady();
    await nextTick();

    await wrapper
      .find("#auth-invitation-password")
      .setValue("Orbit-Research!26");
    await wrapper
      .find("#auth-invitation-confirm")
      .setValue("Orbit-Research!26");
    await wrapper.find("button[type=submit]").trigger("click");

    await flushPromises();
    await nextTick();

    expect(authGateway.acceptInvitation).toHaveBeenCalledWith(
      expect.objectContaining({ token: "tok-1" })
    );
    // 懒加载路由组件的动态 import 是宏任务，等路由真正落定再断言
    await waitForRoute(router, AUTH_PATHS.signingIn);
    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.signingIn);
  });

  it("提供换账号入口，回到登录页", async () => {
    const { wrapper, router } = mountShell(InvitationView);
    await router.isReady();
    await nextTick();

    const link = wrapper
      .findAll("a")
      .find(anchor =>
        anchor
          .text()
          .includes(i18n.global.t("auth.invitation.useAnotherAccount"))
      );
    expect(link).toBeTruthy();
    expect(link!.attributes("href")).toContain(AUTH_PATHS.login);
  });
});
