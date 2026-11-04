// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import HumanApprovalIndicator from "./HumanApprovalIndicator.vue";

describe("HumanApprovalIndicator", () => {
  it("渲染 label 与状态图标", () => {
    const w = mount(HumanApprovalIndicator, {
      props: { state: "approved", label: "Approved" }
    });
    expect(w.text()).toContain("Approved");
    expect(w.find("svg").exists()).toBe(true);
  });

  it("各状态渲染不同图标并落到类名", () => {
    for (const state of [
      "pending",
      "approved",
      "rejected",
      "not-required"
    ] as const) {
      const w = mount(HumanApprovalIndicator, { props: { state } });
      expect(w.html()).toContain(`is-${state}`);
    }
  });

  it("actor 渲染为次要文字", () => {
    const w = mount(HumanApprovalIndicator, {
      props: { state: "approved", label: "Approved", actor: "Lead: Alex" }
    });
    expect(w.text()).toContain("Lead: Alex");
    expect(w.find(".ui-approval__actor").exists()).toBe(true);
  });

  it("role=status", () => {
    const w = mount(HumanApprovalIndicator, { props: {} });
    expect(w.attributes("role")).toBe("status");
  });
});
