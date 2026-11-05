// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";

import SideNav from "./SideNav.vue";
import { useShellStore } from "@/stores/shell";
import { mountShell } from "./test-utils";

/**
 * 左导航两态行为（Figma 7-3454 展开 / 7-3724 折叠）。
 *
 * 断言行为（激活态 / 分组折叠 / 徽标 / 折叠态渲染差异），不锁类名。
 */
describe("SideNav", () => {
  // 折叠态会写 localStorage；不隔离会污染后续用例的初始态
  beforeEach(() => {
    localStorage.clear();
  });

  it("渲染四组导航上下文（个人 / 工作区 / 项目模块 / 管理）", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const labels = wrapper.findAll(".shell-sidenav__group-label");
    expect(labels.map(el => el.text())).toEqual([
      "个人",
      "工作区",
      "项目模块",
      "管理"
    ]);
  });

  it("管理组默认收起，点击分组标题后展开其条目", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const adminItems = () =>
      wrapper
        .findAll(".shell-sidenav__item")
        .filter(el => (el.attributes("href") ?? "").includes("/admin/"));
    // 收起的分组必须真正离开 DOM（否则仍占键盘 Tab 顺序）
    expect(adminItems().length).toBe(0);

    const toggles = wrapper.findAll(".shell-sidenav__group-toggle");
    await toggles[3].trigger("click");
    await nextTick();

    expect(adminItems().length).toBe(3);
  });

  it("当前路由对应的导航项带激活标记", async () => {
    const { wrapper, router } = mountShell(SideNav);
    await router.push("/project/requirements");
    await nextTick();

    const active = wrapper.findAll(".shell-sidenav__item.is-active");
    expect(active.length).toBe(1);
    expect(active[0].attributes("href")).toBe("/project/requirements");
    expect(active[0].attributes("aria-current")).toBe("page");
  });

  it("带徽标的条目渲染徽标数字", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const badges = wrapper.findAll(".shell-sidenav__badge");
    expect(badges.map(el => el.text())).toEqual(["3", "2"]);
  });

  it("展开态显示条目文案并隐藏组分隔线", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    expect(wrapper.find(".shell-sidenav__item-label").exists()).toBe(true);
    expect(wrapper.find(".shell-sidenav__divider").exists()).toBe(false);
  });

  it("折叠态隐藏条目文案，改用图标与未读圆点", async () => {
    const { wrapper } = mountShell(SideNav);
    const shell = useShellStore();
    shell.setSidebarCollapsed(true);
    await nextTick();

    expect(wrapper.find(".shell-sidenav__item-label").exists()).toBe(false);
    expect(wrapper.find(".shell-sidenav__divider").exists()).toBe(true);
    expect(wrapper.find(".shell-sidenav__dot").exists()).toBe(true);
  });

  it("点击折叠开关切换导航折叠态", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const toggle = wrapper.find(".shell-sidenav__control-button");
    expect(toggle.attributes("aria-expanded")).toBe("true");

    await toggle.trigger("click");
    await nextTick();

    expect(toggle.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(".shell-sidenav__item-label").exists()).toBe(false);
  });
});
