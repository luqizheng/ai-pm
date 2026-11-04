// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import AuthPatternsView from "./AuthPatternsView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

/**
 * 认证状态参照板：三组状态齐备（字段与校验 / 动作与交互态 / 反馈与键盘约定）。
 */
describe("AuthPatternsView", () => {
  it("渲染三组状态与键盘约定", async () => {
    const { wrapper, router } = mountShell(AuthPatternsView);
    await router.isReady();
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.fields.title")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.actions.title")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.feedback.title")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.feedback.keyTab")
    );
  });

  it("渲染错误 / 信息 / 过期 / 成功四类反馈", async () => {
    const { wrapper, router } = mountShell(AuthPatternsView);
    await router.isReady();
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.feedback.errorTitle")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.feedback.infoTitle")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.feedback.expiredTitle")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.feedback.success")
    );
  });

  it("列出按钮的全部交互态", async () => {
    const { wrapper, router } = mountShell(AuthPatternsView);
    await router.isReady();
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.actions.hover")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.actions.focus")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.actions.active")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.actions.disabled")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.patterns.actions.loading")
    );
  });
});
