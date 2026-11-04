// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import SidebarItem from "@/components/ui/sidebar-item/SidebarItem.vue";
import { describe, expect, it } from "vitest";

import NavigationGroup from "./NavigationGroup.vue";

describe("NavigationGroup", () => {
  it("展开态渲染标题与子项", () => {
    const w = mount(NavigationGroup, {
      props: { title: "PROJECT" },
      slots: {
        default: {
          components: { SidebarItem },
          template: "<SidebarItem label='Overview' />"
        }
      }
    });
    expect(w.text()).toContain("PROJECT");
    expect(w.text()).toContain("Overview");
    expect(w.attributes("aria-expanded")).toBeUndefined();
    expect(w.find(".ui-nav-group__toggle").attributes("aria-expanded")).toBe(
      "true"
    );
  });

  it("collapsed 隐藏子项并切换 chevron", () => {
    const w = mount(NavigationGroup, {
      props: { title: "PROJECT", defaultCollapsed: true },
      slots: { default: { template: "<div data-test='child' />" } }
    });
    expect(w.find("[data-test='child']").exists()).toBe(false);
    expect(w.find(".ui-nav-group__toggle").attributes("aria-expanded")).toBe(
      "false"
    );
  });

  it("点击标题切换折叠并发出事件", async () => {
    const w = mount(NavigationGroup, { props: { title: "ADMIN" } });
    await w.find(".ui-nav-group__toggle").trigger("click");
    expect(w.emitted("update:collapsed")?.[0]).toEqual([true]);
    expect(w.emitted("toggle")?.[0]).toEqual([true]);
  });

  it("badge 渲染计数", () => {
    const w = mount(NavigationGroup, { props: { title: "ADMIN", badge: 4 } });
    expect(w.text()).toContain("4");
  });

  it("受控 collapsed 优先于 defaultCollapsed", () => {
    const w = mount(NavigationGroup, {
      props: { title: "X", collapsed: false, defaultCollapsed: true }
    });
    expect(w.find(".ui-nav-group__toggle").attributes("aria-expanded")).toBe(
      "true"
    );
  });
});
