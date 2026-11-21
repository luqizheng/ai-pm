// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";

import SideNav from "./SideNav.vue";
import { useShellStore } from "@/stores/shell";
import { i18n } from "@/lib/i18n";
import { mountShell } from "./test-utils";

/**
 * 左导航两态行为（Figma 7-3454 展开 / 7-3724 折叠）+ 三层 IA 分组渲染
 * （Figma「三层」画布 157:3603）。
 *
 * 断言行为（分组顺序 / 项目域条件渲染 / 管理组默认折叠 / 激活态 / 徽标 /
 * 搜索动作项 / 折叠态渲染差异），不锁类名。项目/团队切换器只在顶栏
 * （"CONTEXT SELECTOR · NOT A NAVIGATION LEVEL"），侧栏不渲染切换器，
 * 切换守卫链覆盖见 ProjectSwitcher.test.ts（global）。
 */
describe("SideNav", () => {
  // 折叠态会写 localStorage；不隔离会污染后续用例的初始态
  beforeEach(() => {
    localStorage.clear();
  });

  /** 注入项目上下文（R2-D4：无当前项目时项目域组不渲染） */
  async function mountWithProject() {
    const { wrapper, router } = mountShell(SideNav);
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Project A" },
      { id: "p2", name: "Project B" }
    ]);
    shell.setCurrentProject("p1");
    await nextTick();
    return { wrapper, router, shell };
  }

  it("无当前项目：仅渲染全局/管理两组，项目域组整体隐藏", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const labels = wrapper.findAll(".ui-nav-group__title");
    expect(labels.map(el => el.text())).toEqual(["全局", "管理"]);
    // R2-D4/C06：不渲染项目域条目，也不伪造项目上下文
    const hrefs = wrapper
      .findAll(".ui-sidebar-item")
      .map(el => el.attributes("href"));
    expect(hrefs.some(href => href?.startsWith("/project/"))).toBe(false);
  });

  it("侧栏不内嵌项目切换器（上下文选择器只在顶栏）", async () => {
    const { wrapper } = await mountWithProject();

    expect(wrapper.find(".shell-project-switcher").exists()).toBe(false);
    expect(wrapper.findComponent({ name: "ProjectSwitcher" }).exists()).toBe(
      false
    );
  });

  it("有当前项目：六个业务域分组按 Figma 顺序渲染", async () => {
    const { wrapper } = await mountWithProject();

    const labels = wrapper.findAll(".ui-nav-group__title");
    expect(labels.map(el => el.text())).toEqual([
      "全局",
      "项目总览",
      "开发",
      "知识",
      "环境",
      "管理"
    ]);
  });

  it("开发组条目顺序：仓库（/project/settings/repositories）在前", async () => {
    const { wrapper } = await mountWithProject();

    // 管理组默认折叠，先展开使条目进入 DOM，再断言顺序
    const toggles = wrapper.findAll(".ui-nav-group__toggle");
    await toggles[toggles.length - 1].trigger("click");
    await nextTick();

    const hrefs = wrapper
      .findAll(".ui-sidebar-item")
      .map(el => el.attributes("href"));
    // 项目域首条 = 项目总览
    const firstProjectItem = hrefs.find(href => href?.startsWith("/project/"));
    expect(firstProjectItem).toBe("/project/command-center");
    // Figma DEVELOPMENT 首项 = 仓库
    const developmentIndex = hrefs.indexOf("/project/settings/repositories");
    expect(developmentIndex).toBeGreaterThan(-1);
    expect(hrefs.indexOf("/project/discovery")).toBeGreaterThan(
      developmentIndex
    );
    // 管理组在项目域组之后
    const adminIndex = hrefs.indexOf("/admin/settings");
    expect(adminIndex).toBeGreaterThan(developmentIndex);
  });

  it("撤出主菜单的条目不再渲染（路由保留，命令面板直达）", async () => {
    const { wrapper } = await mountWithProject();

    const hrefs = wrapper
      .findAll(".ui-sidebar-item")
      .map(el => el.attributes("href"));
    for (const path of [
      "/dashboard",
      "/project/design",
      "/project/tasks",
      "/project/analysis",
      "/project/cognition",
      "/project/product-cognition",
      "/project/settings/ai"
    ]) {
      expect(hrefs).not.toContain(path);
    }
  });

  it("当前路由对应的导航项带激活标记", async () => {
    const { wrapper, router } = await mountWithProject();
    await router.push("/project/requirements");
    await nextTick();

    const active = wrapper.findAll(".ui-sidebar-item.is-active");
    expect(active.length).toBe(1);
    expect(active[0].attributes("href")).toBe("/project/requirements");
    expect(active[0].attributes("aria-current")).toBe("page");
  });

  it("主菜单条目无 PLANNED 状态徽标（占位项已撤出主菜单）", async () => {
    const { wrapper } = await mountWithProject();

    const planned = wrapper.findAll(".ui-sidebar-item__badge.is-status");
    expect(planned.length).toBe(0);
  });

  it("开发环境父子路由：父项 redirect 至 Devices 且激活态覆盖子项", async () => {
    const { wrapper, router } = await mountWithProject();
    await router.push("/project/settings/devenv");
    await router.isReady();

    // 父路由 redirect 到 Devices 新路径（旧路径 /project/devenv 兼容重定向同目标）
    expect(router.currentRoute.value.path).toBe(
      "/project/settings/devenv/devices"
    );

    // 父项激活 → 子菜单（Devices/Executors/Runners）可见
    const devenv = wrapper
      .findAll(".ui-sidebar-item")
      .find(el =>
        (el.attributes("href") ?? "").includes("/project/settings/devenv")
      );
    expect(devenv).toBeTruthy();
    expect(devenv!.classes()).toContain("is-active");

    const subitems = wrapper.findAll(".shell-sidenav__subitem");
    const subHrefs = subitems.map(el => el.attributes("href"));
    expect(subHrefs).toEqual([
      "/project/settings/devenv/devices",
      "/project/settings/devenv/executors",
      "/project/settings/runners"
    ]);
    // Executors 为 PLANNED 占位（子项渲染规划标识）
    expect(subitems[1].find(".shell-sidenav__planned").exists()).toBe(true);
  });

  it("管理组默认折叠（Figma ADMINISTRATION Collapsed），展开后条目出现", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const adminItems = () =>
      wrapper
        .findAll(".ui-sidebar-item")
        .filter(el => (el.attributes("href") ?? "").includes("/admin/"));
    // 默认折叠：遗留管理条目离开 DOM（否则仍占键盘 Tab 顺序）
    expect(adminItems().length).toBe(0);

    // 管理组恒为最后一个分组，展开其标题行
    const toggles = wrapper.findAll(".ui-nav-group__toggle");
    await toggles[toggles.length - 1].trigger("click");
    await nextTick();

    // 展开后：设置 + 公告管理两项（成员/AI Provider 已迁团队设置子树）
    expect(adminItems().length).toBe(2);
  });

  it("全局搜索为动作项：仅全局组渲染一条，点击触发 open-search（⌘K 面板）", async () => {
    const { wrapper } = await mountWithProject();

    const searchItems = wrapper
      .findAll(".ui-sidebar-item")
      .filter(el => !el.attributes("href"));
    // 分组 v-for 内的组级守卫：动作项全侧栏唯一，不得随每组重复
    expect(searchItems.length).toBe(1);
    expect(searchItems[0].find(".ui-sidebar-item__label").text()).toBe(
      i18n.global.t("shell.app.side.item.globalSearch")
    );

    await searchItems[0].trigger("click");
    expect(wrapper.emitted("open-search")).toHaveLength(1);
  });

  it("带徽标的条目渲染徽标数字", async () => {
    const { wrapper } = await mountWithProject();

    // 动态徽标仅知识评审（人工审核中心）一项，由 reviewCenter store 随
    // 当前项目注入，聚合不可用或为 0 时不显示——不伪造数字（V1.1 §6.3）。
    const badges = wrapper.findAll(".ui-sidebar-item__badge");
    expect(badges.length).toBe(0);
  });

  it("展开态显示条目文案，分隔线在 Figma 分组边界（无项目：一组一条）", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    expect(wrapper.find(".ui-sidebar-item__label").exists()).toBe(true);
    // 全局│管理 → 1 条分隔线；有项目时全局组后 + 管理组前 → 2 条
    expect(wrapper.findAll(".shell-sidenav__divider").length).toBe(1);
  });

  it("折叠态隐藏条目文案，改用图标与未读圆点", async () => {
    const { wrapper } = mountShell(SideNav);
    const shell = useShellStore();
    shell.setSidebarCollapsed(true);
    await nextTick();

    expect(wrapper.find(".ui-sidebar-item__label").exists()).toBe(false);
    expect(wrapper.findAll(".shell-sidenav__divider").length).toBe(1);
    // 未读圆点现仅知识评审动态徽标驱动（无项目/无聚合时不得伪造圆点）
    expect(wrapper.find(".shell-sidenav__dot").exists()).toBe(false);
  });

  it("点击折叠开关切换导航折叠态", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const toggle = wrapper.find(".shell-sidenav__control-button");
    expect(toggle.attributes("aria-expanded")).toBe("true");

    await toggle.trigger("click");
    await nextTick();

    expect(toggle.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(".ui-sidebar-item__label").exists()).toBe(false);
  });
});
