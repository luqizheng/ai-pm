// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

const { fetchSetupStatus, registerFirstAdmin, authGateway } = vi.hoisted(
  () => ({
    fetchSetupStatus: vi.fn(),
    registerFirstAdmin: vi.fn(),
    authGateway: {
      signIn: vi.fn(),
      signOut: vi.fn(async () => {}),
      requestPasswordReset: vi.fn(async () => {}),
      resetPassword: vi.fn(async () => {}),
      acceptInvitation: vi.fn()
    }
  })
);

vi.mock("@/api/setup", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/setup")>();
  return { ...actual, fetchSetupStatus, registerFirstAdmin };
});

vi.mock("@/api/auth", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/auth")>();
  return { ...actual, authGateway };
});

import { SetupError } from "@/api/setup";
import SetupView from "./SetupView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import { AUTH_PATHS } from "@/router/auth";
import { waitForRoute } from "@/lib/test-utils";

const SESSION = {
  account: "admin",
  organization: "AI R&D Workspace",
  user: {
    id: "u-admin",
    username: "admin",
    nickname: "Admin",
    email: null,
    phone: null,
    avatar: null,
    roles: ["tenant_admin"],
    permissions: []
  }
};

/**
 * 初始化注册页（空库强制注册首个管理员）行为：
 *   - 挂载探测：系统已初始化 → 送回登录页；探测失败 → 停留本页；
 *   - 字段级校验（必填 / 口令 ≥8 / 两次一致 / 可选邮箱格式）；
 *   - 成功注册 → 自动登录 → 认证进度页（后续走 onboarding 建团）；
 *   - 403（注册通道已关闭）/ 409（用户名重复）/ 网络 → 对应反馈。
 */
describe("SetupView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    fetchSetupStatus.mockResolvedValue({ setup_required: true, user_count: 0 });
    authGateway.signIn.mockResolvedValue(SESSION);
  });

  async function mountView() {
    const mounted = mountShell(SetupView);
    await mounted.router.isReady();
    await flushPromises();
    await nextTick();
    return mounted;
  }

  it("挂载探测：系统已初始化时送回登录页", async () => {
    fetchSetupStatus.mockResolvedValue({
      setup_required: false,
      user_count: 3
    });
    const { router } = await mountView();
    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.login);
  });

  it("挂载探测失败（网关不可达）时停留在注册页", async () => {
    fetchSetupStatus.mockResolvedValue(null);
    const { router } = await mountView();
    expect(router.currentRoute.value.path).not.toBe(AUTH_PATHS.login);
  });

  it("空提交时报出必填字段级错误", async () => {
    const { wrapper } = await mountView();

    await wrapper.find("form").trigger("submit");
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.validation.usernameRequired")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.validation.nicknameRequired")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.validation.passwordRequired")
    );
    expect(registerFirstAdmin).not.toHaveBeenCalled();
  });

  it("口令过短与两次不一致分别报错", async () => {
    const { wrapper } = await mountView();

    await wrapper.find("#auth-setup-username").setValue("admin");
    await wrapper.find("#auth-setup-nickname").setValue("Admin");
    await wrapper.find("#auth-setup-password").setValue("short");
    await wrapper.find("#auth-setup-confirm").setValue("short2");
    await wrapper.find("form").trigger("submit");
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.validation.passwordTooShort")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.validation.mismatch")
    );
    expect(registerFirstAdmin).not.toHaveBeenCalled();
  });

  it("可选邮箱格式非法时报错", async () => {
    const { wrapper } = await mountView();

    await wrapper.find("#auth-setup-username").setValue("admin");
    await wrapper.find("#auth-setup-nickname").setValue("Admin");
    await wrapper.find("#auth-setup-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-setup-confirm").setValue("Orbit-Research!26");
    await wrapper.find("#auth-setup-email").setValue("admin@");
    await wrapper.find("form").trigger("submit");
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.validation.emailInvalid")
    );
    expect(registerFirstAdmin).not.toHaveBeenCalled();
  });

  it("注册成功后自动登录并进入认证进度页", async () => {
    const { wrapper, router } = await mountView();

    await wrapper.find("#auth-setup-username").setValue("admin");
    await wrapper.find("#auth-setup-nickname").setValue("Admin");
    await wrapper.find("#auth-setup-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-setup-confirm").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");

    await flushPromises();

    expect(registerFirstAdmin).toHaveBeenCalledTimes(1);
    expect(registerFirstAdmin).toHaveBeenCalledWith({
      username: "admin",
      nickname: "Admin",
      password: "Orbit-Research!26"
    });
    expect(authGateway.signIn).toHaveBeenCalledWith({
      account: "admin",
      password: "Orbit-Research!26",
      remember: true
    });
    await waitForRoute(router, AUTH_PATHS.signingIn);
    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.signingIn);
  });

  it("注册成功但自动登录失败时给出可执行的指引", async () => {
    authGateway.signIn.mockRejectedValue(new Error("boom"));
    const { wrapper } = await mountView();

    await wrapper.find("#auth-setup-username").setValue("admin");
    await wrapper.find("#auth-setup-nickname").setValue("Admin");
    await wrapper.find("#auth-setup-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-setup-confirm").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.autoSignInFailedTitle")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.autoSignInFailedDetail")
    );
  });

  it("系统已初始化（403）时展示关闭反馈与返回登录入口", async () => {
    registerFirstAdmin.mockRejectedValue(new SetupError("closed"));
    const { wrapper } = await mountView();

    await wrapper.find("#auth-setup-username").setValue("admin");
    await wrapper.find("#auth-setup-nickname").setValue("Admin");
    await wrapper.find("#auth-setup-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-setup-confirm").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain(i18n.global.t("auth.setup.closedTitle"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.setup.closedDetail"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.setup.backToSignIn"));
  });

  it("用户名重复（409）映射为字段级错误", async () => {
    registerFirstAdmin.mockRejectedValue(new SetupError("duplicate"));
    const { wrapper } = await mountView();

    await wrapper.find("#auth-setup-username").setValue("admin");
    await wrapper.find("#auth-setup-nickname").setValue("Admin");
    await wrapper.find("#auth-setup-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-setup-confirm").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.setup.validation.duplicate")
    );
  });

  it("网络失败展示网关级反馈", async () => {
    registerFirstAdmin.mockRejectedValue(new SetupError("network"));
    const { wrapper } = await mountView();

    await wrapper.find("#auth-setup-username").setValue("admin");
    await wrapper.find("#auth-setup-nickname").setValue("Admin");
    await wrapper.find("#auth-setup-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-setup-confirm").setValue("Orbit-Research!26");
    await wrapper.find("form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain(i18n.global.t("auth.setup.errorTitle"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.setup.errorDetail"));
  });
});
