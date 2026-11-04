// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import StatusIndicator from "./StatusIndicator.vue";

describe("StatusIndicator", () => {
  it("默认渲染圆点，无标签", () => {
    const w = mount(StatusIndicator, { props: {} });
    expect(w.find(".ui-status__dot").exists()).toBe(true);
    expect(w.find(".ui-status__label").exists()).toBe(false);
  });

  it("label 渲染文本", () => {
    const w = mount(StatusIndicator, { props: { label: "Environment ready" } });
    expect(w.text()).toContain("Environment ready");
  });

  it("status/size 落到类名", () => {
    const w = mount(StatusIndicator, {
      props: { status: "success", size: "lg" }
    });
    expect(w.html()).toContain("ui-status--success");
    expect(w.html()).toContain("ui-status--lg");
  });

  it("role=status", () => {
    const w = mount(StatusIndicator, { props: {} });
    expect(w.attributes("role")).toBe("status");
  });
});
