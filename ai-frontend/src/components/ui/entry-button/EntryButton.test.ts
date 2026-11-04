// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import EntryButton from "./EntryButton.vue";

describe("EntryButton", () => {
  it("渲染 label 与 sparkles 图标", () => {
    const w = mount(EntryButton, { props: { label: "Ask AI" } });
    expect(w.text()).toContain("Ask AI");
    expect(w.find("svg").exists()).toBe(true);
  });

  it("loading 时禁用并标记 aria-busy", () => {
    const w = mount(EntryButton, { props: { label: "Ask AI", loading: true } });
    expect(w.attributes("disabled")).toBeDefined();
    expect(w.attributes("aria-busy")).toBe("true");
  });

  it("variant/size 落到类名", () => {
    const w = mount(EntryButton, {
      props: { label: "AI", variant: "accent", size: "lg" }
    });
    expect(w.html()).toContain("ui-entry-button--accent");
    expect(w.html()).toContain("ui-entry-button--lg");
  });

  it("点击发出 click", async () => {
    const w = mount(EntryButton, { props: { label: "Ask AI" } });
    await w.trigger("click");
    expect(w.emitted("click")).toHaveLength(1);
  });
});
