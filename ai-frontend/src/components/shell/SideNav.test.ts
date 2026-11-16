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

  it("管理组默认展开与其他组同级，点击分组标题可收起", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const adminItems = () =>
      wrapper
        .findAll(".shell-sidenav__item")
        .filter(el => (el.attributes("href") ?? "").includes("/admin/"));
    // 默认展开：管理组与其他分组同级呈现，不收成一行小标题
    expect(adminItems().length).toBe(3);

    const toggles = wrapper.findAll(".shell-sidenav__group-toggle");
    await toggles[3].trigger("click");
    await nextTick();

    // 收起后条目真正离开 DOM（否则仍占键盘 Tab 顺序）
    expect(adminItems().length).toBe(0);
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

    // 徽标数字来自导航树静态配置（非实时计数）。AI 执行中心原先的
    // 「2」是演示期硬编码占位，S05 接真后由 execution-overview 实时聚合
    // 承载，导航上不再放假的固定数字。
    const badges = wrapper.findAll(".shell-sidenav__badge");
    expect(badges.map(el => el.text())).toEqual(["3"]);
  });

  it("展开态显示条目文案，分隔线在区域边界（Personal│Team│项目区│遗留管理）", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    expect(wrapper.find(".shell-sidenav__item-label").exists()).toBe(true);
    // V1.2 §6.1：区域边界各一条（个人│工作区│项目模块│管理 → 3 条）
    expect(wrapper.findAll(".shell-sidenav__divider").length).toBe(3);
  });

  it("项目总览已更名为团队总览（Q-08：只改文案不改路径）", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const item = wrapper
      .findAll(".shell-sidenav__item")
      .find(el => (el.attributes("href") ?? "").includes("/dashboard"));
    expect(item).toBeTruthy();
    expect(item!.find(".shell-sidenav__item-label").text()).toBe("团队总览");
  });

  it("折叠态隐藏条目文案，改用图标与未读圆点", async () => {
    const { wrapper } = mountShell(SideNav);
    const shell = useShellStore();
    shell.setSidebarCollapsed(true);
    await nextTick();

    expect(wrapper.find(".shell-sidenav__item-label").exists()).toBe(false);
    // 折叠态：除首组外组间全加（4 组 → 3 条）
    expect(wrapper.findAll(".shell-sidenav__divider").length).toBe(3);
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
