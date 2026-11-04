// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import AuthLayout from "./AuthLayout.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

/**
 * 认证页外壳：页头（产品标识 + 安全提示）、内容槽、页脚（版本 + 支持链接）。
 */
describe("AuthLayout", () => {
  it("渲染页头标识与右侧安全提示", async () => {
    const { wrapper } = mountShell(AuthLayout);
    await nextTick();

    expect(wrapper.text()).toContain(i18n.global.t("auth.chrome.product"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.chrome.section"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.chrome.secure"));
  });

  it("渲染页脚版本号与支持链接", async () => {
    const { wrapper } = mountShell(AuthLayout);
    await nextTick();

    expect(wrapper.text()).toContain(i18n.global.t("auth.chrome.version"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.chrome.help"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.chrome.privacy"));
    expect(wrapper.text()).toContain(i18n.global.t("auth.chrome.terms"));
  });

  it("内容区渲染传入的插槽", async () => {
    const { wrapper } = mountShell(AuthLayout, {
      slots: { default: "<p>面板内容</p>" }
    });
    await nextTick();

    expect(wrapper.text()).toContain("面板内容");
  });
});
