// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

import ResetPasswordView from "./ResetPasswordView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

/**
 * 重置口令页行为：口令要求实时勾选、两处不一致报错、成功后给出成功态。
 */
describe("ResetPasswordView", () => {
  it("口令要求清单随输入实时勾选", async () => {
    const { wrapper, router } = mountShell(ResetPasswordView);
    await router.isReady();
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.reset.requirementLength")
    );

    await wrapper.find("#auth-reset-password").setValue("Orbit-Research!26");
    await nextTick();

    // 三条要求全部满足
    expect(wrapper.findAll("[data-met=true]").length).toBe(3);
  });

  it("两次口令不一致时报错并给出不一致提示", async () => {
    const { wrapper, router } = mountShell(ResetPasswordView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-reset-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-reset-confirm").setValue("Orbit-Research!27");
    await wrapper.find("button[type=submit]").trigger("click");
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.reset.validation.mismatch")
    );
  });

  it("两次口令一致时给出确认提示", async () => {
    const { wrapper, router } = mountShell(ResetPasswordView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-reset-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-reset-confirm").setValue("Orbit-Research!26");
    await nextTick();

    expect(wrapper.text()).toContain(i18n.global.t("auth.reset.matchHint"));
  });

  it("提交成功后展示口令已更新", async () => {
    const { wrapper, router } = mountShell(ResetPasswordView);
    await router.isReady();
    await nextTick();

    await wrapper.find("#auth-reset-password").setValue("Orbit-Research!26");
    await wrapper.find("#auth-reset-confirm").setValue("Orbit-Research!26");
    await wrapper.find("button[type=submit]").trigger("click");

    await new Promise(resolve => setTimeout(resolve, 600));
    await flushPromises();
    await nextTick();

    expect(wrapper.text()).toContain(i18n.global.t("auth.reset.successTitle"));
  });
});
