// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

import ForgotPasswordView from "./ForgotPasswordView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

/**
 * 找回口令页行为：邮箱校验、提交后回到「查收邮件」信息态。
 */
describe("ForgotPasswordView", () => {
  it("空邮箱提交时报出必填错误", async () => {
    const { wrapper, router } = mountShell(ForgotPasswordView);
    await router.isReady();
    await nextTick();

    await wrapper.find("button[type=submit]").trigger("click");
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.forgot.validation.emailRequired")
    );
  });

  it("非法邮箱格式被拦下", async () => {
    const { wrapper, router } = mountShell(ForgotPasswordView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-forgot-email").setValue("not-an-email");
    await wrapper.find("button[type=submit]").trigger("click");
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.forgot.validation.emailInvalid")
    );
  });

  it("提交成功后提示查收邮件，并给出换邮箱出口", async () => {
    const { wrapper, router } = mountShell(ForgotPasswordView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-forgot-email").setValue("alex.morgan@acme.dev");
    await wrapper.find("button[type=submit]").trigger("click");

    await new Promise(resolve => setTimeout(resolve, 600));
    await flushPromises();
    await nextTick();

    expect(wrapper.text()).toContain(i18n.global.t("auth.forgot.successTitle"));
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.forgot.useAnotherEmail")
    );
  });
});
