// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { Folder } from "lucide-vue-next";
import { describe, expect, it } from "vitest";

import SidebarItem from "./SidebarItem.vue";

describe("SidebarItem", () => {
  it("渲染 label 与图标", () => {
    const w = mount(SidebarItem, {
      props: { label: "Projects", icon: Folder }
    });
    expect(w.text()).toContain("Projects");
    expect(w.find("svg").exists()).toBe(true);
  });

  it("active 时标记 aria-current 并渲染指示条", () => {
    const w = mount(SidebarItem, {
      props: { label: "Overview", state: "active" }
    });
    expect(w.attributes("aria-current")).toBe("page");
    expect(w.find(".ui-sidebar-item__indicator").exists()).toBe(true);
  });

  it("collapsed 时隐藏 label 与徽标", () => {
    const w = mount(SidebarItem, {
      props: { label: "Projects", collapsed: true, badge: 3 }
    });
    expect(w.text()).not.toContain("Projects");
    expect(w.find(".ui-sidebar-item__badge").exists()).toBe(false);
  });

  it("badge 计数与状态两种类型都渲染", () => {
    const count = mount(SidebarItem, { props: { label: "A", badge: 8 } });
    expect(count.text()).toContain("8");
    const status = mount(SidebarItem, {
      props: { label: "B", badge: "Ready", badgeType: "status" }
    });
    expect(status.text()).toContain("Ready");
  });

  it("restricted 态禁用按钮", () => {
    const w = mount(SidebarItem, {
      props: { label: "Admin", state: "restricted" }
    });
    expect(w.attributes("disabled")).toBeDefined();
  });

  it("loading 态标记 aria-busy", () => {
    const w = mount(SidebarItem, {
      props: { label: "Sync", state: "loading" }
    });
    expect(w.attributes("aria-busy")).toBe("true");
  });
});
