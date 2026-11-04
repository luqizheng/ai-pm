// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { Trash2 } from "lucide-vue-next";
import { describe, expect, it } from "vitest";

import MenuItem from "./MenuItem.vue";

describe("MenuItem", () => {
  it("渲染 label 与快捷键 chip", () => {
    const w = mount(MenuItem, {
      props: { label: "Duplicate", shortcut: "⌘ D" }
    });
    expect(w.text()).toContain("Duplicate");
    expect(w.find("kbd").text()).toBe("⌘ D");
  });

  it("shortcut 空串隐藏 chip", () => {
    const w = mount(MenuItem, { props: { label: "A", shortcut: "" } });
    expect(w.find("kbd").exists()).toBe(false);
  });

  it("selected 标记 aria-selected", () => {
    const w = mount(MenuItem, { props: { label: "A", selected: true } });
    expect(w.attributes("aria-selected")).toBe("true");
  });

  it("disabled 禁用且不发事件", async () => {
    const w = mount(MenuItem, { props: { label: "A", disabled: true } });
    expect(w.attributes("disabled")).toBeDefined();
  });

  it("danger + icon 渲染", () => {
    const w = mount(MenuItem, {
      props: { label: "Delete", icon: Trash2, danger: true }
    });
    expect(w.find("svg").exists()).toBe(true);
    expect(w.html()).toContain("is-danger");
  });

  it("点击发出 click", async () => {
    const w = mount(MenuItem, { props: { label: "A" } });
    await w.trigger("click");
    expect(w.emitted("click")).toHaveLength(1);
  });
});
