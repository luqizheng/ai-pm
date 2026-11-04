// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import ProgressIndicator from "./ProgressIndicator.vue";

describe("ProgressIndicator", () => {
  it("value=40 max=100 时 aria-valuenow=40 且宽度 40%", () => {
    const w = mount(ProgressIndicator, { props: { value: 40 } });
    expect(w.attributes("aria-valuenow")).toBe("40");
    const bar = w.find(".ui-progress__bar");
    expect(bar.attributes("style")).toContain("40%");
  });

  it("value 超界收敛到 0~100", () => {
    const high = mount(ProgressIndicator, { props: { value: 250 } });
    expect(high.attributes("aria-valuenow")).toBe("100");
    const low = mount(ProgressIndicator, { props: { value: -5 } });
    expect(low.attributes("aria-valuenow")).toBe("0");
  });

  it("label 渲染", () => {
    const w = mount(ProgressIndicator, { props: { label: "Signing in…" } });
    expect(w.text()).toContain("Signing in…");
  });

  it("indeterminate 无 aria-valuenow 且标记 aria-busy", () => {
    const w = mount(ProgressIndicator, { props: { indeterminate: true } });
    expect(w.attributes("aria-valuenow")).toBeUndefined();
    expect(w.attributes("aria-busy")).toBe("true");
  });

  it("variant 落到类名", () => {
    const w = mount(ProgressIndicator, { props: { variant: "danger" } });
    expect(w.html()).toContain("ui-progress--danger");
  });
});
