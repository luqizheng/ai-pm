// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import AiStatusIndicator from "./AiStatusIndicator.vue";

describe("AiStatusIndicator", () => {
  it("默认渲染点，无标签", () => {
    const w = mount(AiStatusIndicator, { props: {} });
    expect(w.find(".ui-ai-status__dot").exists()).toBe(true);
    expect(w.find(".ui-ai-status__label").exists()).toBe(false);
  });

  it("label 渲染", () => {
    const w = mount(AiStatusIndicator, {
      props: { status: "thinking", label: "Analyzing…" }
    });
    expect(w.text()).toContain("Analyzing…");
    expect(w.html()).toContain("is-thinking");
  });

  it("各状态类名", () => {
    for (const status of ["idle", "waiting", "done", "failed"] as const) {
      const w = mount(AiStatusIndicator, { props: { status } });
      expect(w.html()).toContain(`is-${status}`);
    }
  });

  it("size 三档落类名", () => {
    const w = mount(AiStatusIndicator, { props: { size: "lg" } });
    expect(w.html()).toContain("ui-ai-status--lg");
  });

  it("role=status", () => {
    const w = mount(AiStatusIndicator, { props: {} });
    expect(w.attributes("role")).toBe("status");
  });
});
