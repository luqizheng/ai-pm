// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

import LoginView from "./LoginView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import { AUTH_PATHS } from "@/router/auth";
import { waitForRoute } from "@/lib/test-utils";

/**
 * 登录页行为：字段级校验、凭据错误反馈、成功后进入认证进度页。
 */
describe("LoginView", () => {
  it("空提交时给出两个字段级错误", async () => {
    const { wrapper, router } = mountShell(LoginView);
    await router.isReady();
    await nextTick();

    await wrapper.find("button[type=submit]").trigger("click");
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
    await wrapper.find("button[type=submit]").trigger("click");
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
    await wrapper.find("button[type=submit]").trigger("click");

    // mock 网关有 700ms 延迟，等它返回
    await new Promise(resolve => setTimeout(resolve, 900));
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
    await wrapper.find("button[type=submit]").trigger("click");

    await waitForRoute(router, AUTH_PATHS.signingIn);

    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.signingIn);
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
