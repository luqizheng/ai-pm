// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { Bell } from "lucide-vue-next";
import { describe, expect, it } from "vitest";

import TopBarItem from "./TopBarItem.vue";

describe("TopBarItem", () => {
  it("渲染 label / 图标 / 快捷键 chip", () => {
    const w = mount(TopBarItem, {
      props: { label: "Notifications", icon: Bell, shortcut: "⌘ N" }
    });
    expect(w.text()).toContain("Notifications");
    expect(w.find("svg").exists()).toBe(true);
    expect(w.find("kbd").text()).toBe("⌘ N");
  });

  it("shortcut 空串隐藏 chip", () => {
    const w = mount(TopBarItem, { props: { label: "Help", shortcut: "" } });
    expect(w.find("kbd").exists()).toBe(false);
  });

  it("active 标记 aria-current", () => {
    const w = mount(TopBarItem, { props: { label: "AI", active: true } });
    expect(w.attributes("aria-current")).toBe("page");
  });

  it("点击发出 click", async () => {
    const w = mount(TopBarItem, { props: { label: "Help" } });
    await w.trigger("click");
    expect(w.emitted("click")).toHaveLength(1);
  });

  it("disabled 禁用", () => {
    const w = mount(TopBarItem, { props: { label: "X", disabled: true } });
    expect(w.attributes("disabled")).toBeDefined();
  });
});
