// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";

import ProjectSwitcher from "./ProjectSwitcher.vue";
import SideNav from "./SideNav.vue";
import { useShellStore } from "@/stores/shell";
import { i18n } from "@/lib/i18n";
import { mountShell } from "./test-utils";

/**
 * 左导航两态行为（Figma 7-3454 展开 / 7-3724 折叠）+ R2-D4 项目区条件渲染。
 *
 * 断言行为（激活态 / 分组折叠 / 徽标 / 折叠态渲染差异 / 项目区与切换守卫链），
 * 不锁类名。
 */
describe("SideNav", () => {
  // 折叠态会写 localStorage；不隔离会污染后续用例的初始态
  beforeEach(() => {
    localStorage.clear();
  });

  /** 注入项目上下文（R2-D4：无当前项目时项目区不渲染） */
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

  it("无当前项目：仅渲染 Personal/Team/遗留管理三组，项目区整体隐藏", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const labels = wrapper.findAll(".shell-sidenav__group-label");
    expect(labels.map(el => el.text())).toEqual(["个人", "工作区", "管理"]);
    // R2-D4/C06：不渲染空项目菜单（紧凑项目头与项目模块条目都不出现）
    expect(wrapper.find(".shell-project-switcher").exists()).toBe(false);
    const hrefs = wrapper
      .findAll(".shell-sidenav__item")
      .map(el => el.attributes("href"));
    expect(hrefs.some(href => href?.startsWith("/project/"))).toBe(false);
  });

  it("项目总览已更名为团队总览（Q-08：只改文案不改路径）", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const item = wrapper
      .findAll(".shell-sidenav__item")
      .find(el => (el.attributes("href") ?? "").includes("/dashboard"));
    expect(item).toBeTruthy();
    expect(item!.find(".shell-sidenav__item-label").text()).toBe(
      i18n.global.t("shell.app.side.item.dashboard")
    );
  });

  it("有当前项目：渲染紧凑项目头与五组结构（含项目设置置底）", async () => {
    const { wrapper } = await mountWithProject();

    // 紧凑 Project Switcher（R2-D3：header 变体复用）
    expect(wrapper.find(".shell-project-switcher.is-header").exists()).toBe(
      true
    );

    const labels = wrapper.findAll(".shell-sidenav__group-label");
    expect(labels.map(el => el.text())).toEqual([
      "个人",
      "工作区",
      "项目模块",
      "项目设置",
      "管理"
    ]);
  });

  it("项目导航顺序：总控台置顶，项目设置在遗留管理之前（R2-D5/C07）", async () => {
    const { wrapper } = await mountWithProject();

    const hrefs = wrapper
      .findAll(".shell-sidenav__item")
      .map(el => el.attributes("href"));
    // 项目区首条 = 项目总控台（全局区 Personal/Team 在前）
    const firstProjectItem = hrefs.find(href => href?.startsWith("/project/"));
    expect(firstProjectItem).toBe("/project/command-center");
    const reposIndex = hrefs.indexOf("/project/settings/repositories");
    const adminIndex = hrefs.indexOf("/admin/settings");
    expect(reposIndex).toBeGreaterThan(-1);
    expect(adminIndex).toBeGreaterThan(reposIndex);
    // 设置组收尾为 AI 配置（V1.2 §5 子树含 AI 配置与执行策略）
    const aiIndex = hrefs.indexOf("/project/settings/ai");
    expect(aiIndex).toBeGreaterThan(reposIndex);
    expect(aiIndex).toBeLessThan(adminIndex);
  });

  it("当前路由对应的导航项带激活标记", async () => {
    const { wrapper, router } = await mountWithProject();
    await router.push("/project/requirements");
    await nextTick();

    const active = wrapper.findAll(".shell-sidenav__item.is-active");
    expect(active.length).toBe(1);
    expect(active[0].attributes("href")).toBe("/project/requirements");
    expect(active[0].attributes("aria-current")).toBe("page");
  });

  it("PLANNED 占位项渲染规划标识（Analysis / Product Cognition / AI 配置）", async () => {
    const { wrapper } = await mountWithProject();

    const planned = wrapper.findAll(".shell-sidenav__planned");
    // 业务模块 2 项（Analysis / Product Cognition）+ 项目设置 3 项
    // （基础设置 / 成员 / AI 配置；开发环境标识在其子项）
    expect(planned.length).toBe(5);
    expect(planned[0].text()).toBe(
      i18n.global.t("shell.app.side.plannedBadge")
    );
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
      .findAll(".shell-sidenav__item")
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

  it("项目切换复用现有守卫链（requestProjectSwitch）", async () => {
    const { wrapper, shell } = await mountWithProject();

    const switcher = wrapper.findComponent(ProjectSwitcher);
    expect(switcher.exists()).toBe(true);
    switcher.vm.$emit("select", "p2");
    await nextTick();

    // 无活跃会话 → requestProjectSwitch 直接落地两阶段切换
    expect(shell.currentProjectId).toBe("p2");
  });

  it("管理组默认展开与其他组同级，点击分组标题可收起", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    const adminItems = () =>
      wrapper
        .findAll(".shell-sidenav__item")
        .filter(el => (el.attributes("href") ?? "").includes("/admin/"));
    // 默认展开：管理组与其他分组同级呈现，不收成一行小标题
    // （C4 后遗留管理组仅剩平台设置/公告两项；成员与 AI Provider 已迁团队设置）
    expect(adminItems().length).toBe(2);

    const toggles = wrapper.findAll(".shell-sidenav__group-toggle");
    await toggles[2].trigger("click");
    await nextTick();

    // 收起后条目真正离开 DOM（否则仍占键盘 Tab 顺序）
    expect(adminItems().length).toBe(0);
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

  it("展开态显示条目文案，分隔线在区域边界（无项目：三区两条）", async () => {
    const { wrapper } = mountShell(SideNav);
    await nextTick();

    expect(wrapper.find(".shell-sidenav__item-label").exists()).toBe(true);
    // V1.2 §6.1：区域边界各一条（个人│工作区│遗留管理 → 无项目区时 2 条）
    expect(wrapper.findAll(".shell-sidenav__divider").length).toBe(2);
  });

  it("折叠态隐藏条目文案，改用图标与未读圆点", async () => {
    const { wrapper } = mountShell(SideNav);
    const shell = useShellStore();
    shell.setSidebarCollapsed(true);
    await nextTick();

    expect(wrapper.find(".shell-sidenav__item-label").exists()).toBe(false);
    // 折叠态：除首组外组间全加（3 组 → 2 条）
    expect(wrapper.findAll(".shell-sidenav__divider").length).toBe(2);
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
