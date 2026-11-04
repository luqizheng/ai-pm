// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import HumanApprovalIndicator from "./HumanApprovalIndicator.vue";
import { i18n } from "@/lib/i18n";

/**
 * 人工批准指示器（DS/AI/HumanApprovalIndicator 三态）。
 * 红线：Approved 显式声明「非 AI 批准」，Rejected 显式声明「未授权任何变更」。
 */
describe("HumanApprovalIndicator", () => {
  it("pending：需要人工确认 + 待决说明", () => {
    const wrapper = mount(HumanApprovalIndicator, {
      props: { state: "pending" },
      global: { plugins: [i18n] }
    });

    expect(wrapper.classes()).toContain("is-pending");
    expect(wrapper.find(".approval-indicator__title").text()).toBe(
      "需要人工确认"
    );
    expect(wrapper.find(".approval-indicator__desc").text()).toContain("待决");
  });

  it("approved：人工已批准 + 非 AI 批准声明", () => {
    const wrapper = mount(HumanApprovalIndicator, {
      props: { state: "approved", actor: "Maya" },
      global: { plugins: [i18n] }
    });

    expect(wrapper.classes()).toContain("is-approved");
    expect(wrapper.find(".approval-indicator__title").text()).toBe(
      "人工已批准"
    );
    expect(wrapper.find(".approval-indicator__desc").text()).toBe(
      "Maya 已批准该请求 · 非 AI 批准"
    );
  });

  it("rejected：人工已驳回 + 未授权声明", () => {
    const wrapper = mount(HumanApprovalIndicator, {
      props: { state: "rejected", actor: "Maya" },
      global: { plugins: [i18n] }
    });

    expect(wrapper.classes()).toContain("is-rejected");
    expect(wrapper.find(".approval-indicator__title").text()).toBe(
      "人工已驳回"
    );
    expect(wrapper.find(".approval-indicator__desc").text()).toBe(
      "Maya 已驳回该请求 · 未授权任何变更"
    );
  });
});
