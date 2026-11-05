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

import { AuthError } from "@/api/auth";
import LoginView from "./LoginView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import { AUTH_PATHS } from "@/router/auth";
import { waitForRoute } from "@/lib/test-utils";

const SESSION = {
  account: "alex.morgan@acme.dev",
  organization: "Acme Research",
  user: {
    id: "u-1",
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
 * 登录页行为：字段级校验、凭据错误反馈、成功后进入认证进度页。
 * 凭据组是原生 <form>：按钮点击与输入框内按 Enter 都走 submit 通道，
 * 测试在 form 上派发 submit 事件（与 ProjectCreateView 同范式）。
 */
describe("LoginView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    authGateway.signIn.mockImplementation(
      async (input: { account: string }) => {
        if (input.account === "locked@acme.dev") {
          throw new AuthError("invalid_credentials");
        }
        return SESSION;
      }
    );
  });
  it("空提交时给出两个字段级错误", async () => {
    const { wrapper, router } = mountShell(LoginView);
    await router.isReady();
    await nextTick();

    await wrapper.find("form").trigger("submit");
    await nextTick();

    // 测试挂载不 attachTo document，document.activeElement 不可断言；
    // 行为契约 = 两个字段级错误同时展示
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.login.validation.accountRequired")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.login.validation.passwordRequired")
    );
  });

  it("账号格式非法时报出邮箱格式错误", async () => {
    const { wrapper, router } = mountShell(LoginView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-login-account").setValue("alex@");
    await wrapper.find("#auth-login-password").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.login.validation.accountInvalid")
    );
  });

  it("凭据错误时展示网关级反馈", async () => {
    const { wrapper, router } = mountShell(LoginView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-login-account").setValue("locked@acme.dev");
    await wrapper.find("#auth-login-password").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");

    await flushPromises();
    await nextTick();

    expect(wrapper.text()).toContain(i18n.global.t("auth.login.errorTitle"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.login.errorDetail"));
  });

  it("登录成功后进入认证进度页", async () => {
    const { wrapper, router } = mountShell(LoginView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-login-account").setValue("alex.morgan@acme.dev");
    await wrapper.find("#auth-login-password").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");

    await waitForRoute(router, AUTH_PATHS.signingIn);

    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.signingIn);
  });

  it("表单提交（含输入框内按 Enter 的隐式提交）调用登录网关", async () => {
    const { wrapper, router } = mountShell(LoginView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-login-account").setValue("alex.morgan@acme.dev");
    await wrapper.find("#auth-login-password").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");

    await flushPromises();

    expect(authGateway.signIn).toHaveBeenCalledTimes(1);
    expect(authGateway.signIn).toHaveBeenCalledWith({
      account: "alex.morgan@acme.dev",
      password: "Orbit-Research!26",
      remember: false
    });
  });

  it("提供单点登录入口（尚未开通时给出说明）", async () => {
    const { wrapper, router } = mountShell(LoginView);
    await router.isReady();
    await nextTick();

    const sso = wrapper
      .findAll("button")
      .find(button => button.text().includes(i18n.global.t("auth.login.sso")));
    expect(sso).toBeTruthy();

    await sso!.trigger("click");
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.login.ssoUnavailable")
    );
  });
});
