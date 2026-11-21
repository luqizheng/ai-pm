import { describe, expect, it } from "vitest";
import { ScanSearch } from "lucide-vue-next";

import {
  auxiliaryNavItems,
  flatNavItems,
  isNavChildActive,
  isNavItemActive,
  mainNavGroups,
  type NavChild,
  type NavItem
} from "./navigation";

/**
 * 导航树契约（壳与路由的单源；Figma「三层」画布 157:3603 IA）。
 */
describe("navigation", () => {
  it("主菜单组按 Figma 三层 IA 排列：全局→项目总览→开发→知识→环境→管理", () => {
    expect(mainNavGroups.map(g => g.id)).toEqual([
      "global",
      "projectOverview",
      "development",
      "knowledge",
      "environment",
      "administration"
    ]);
  });

  it("项目域四组标记 projectScoped；管理组默认折叠（Figma Collapsed）", () => {
    expect(mainNavGroups.filter(g => g.projectScoped).map(g => g.id)).toEqual([
      "projectOverview",
      "development",
      "knowledge",
      "environment"
    ]);
    const administration = mainNavGroups.find(
      g => g.id === "administration"
    )!;
    expect(administration.defaultCollapsed).toBe(true);
    // 非项目域组不默认折叠
    expect(
      mainNavGroups
        .filter(g => g.id !== "administration")
        .every(g => g.defaultCollapsed !== true)
    ).toBe(true);
  });

  it("全局组：个人工作区 / 收件箱 / 项目（全局搜索为渲染层动作项，不占数据位）", () => {
    const global = mainNavGroups.find(g => g.id === "global")!;
    expect(global.items.map(i => i.path)).toEqual([
      "/personal",
      "/notifications",
      "/projects"
    ]);
  });

  it("开发组 Figma 条目顺序：仓库（manualRoute）→ 代码发现 → 需求 → AI 执行", () => {
    const development = mainNavGroups.find(g => g.id === "development")!;
    expect(development.items.map(i => i.path)).toEqual([
      "/project/settings/repositories",
      "/project/discovery",
      "/project/requirements",
      "/project/execution"
    ]);
    // 仓库路径在 /project/settings 前缀下：路由手工注册，助手隐藏语义随路径
    expect(development.items[0].manualRoute).toBe(true);
    expect(
      development.items.slice(1).every(i => i.manualRoute !== true)
    ).toBe(true);
  });

  it("知识组：知识库 + 知识评审（原人工审核中心，动态徽标条目）", () => {
    const knowledge = mainNavGroups.find(g => g.id === "knowledge")!;
    expect(knowledge.items.map(i => i.path)).toEqual([
      "/project/knowledge",
      "/project/review-center"
    ]);
  });

  it("环境组：开发环境带子菜单且为手工路由", () => {
    const environment = mainNavGroups.find(g => g.id === "environment")!;
    expect(environment.items.map(i => i.path)).toEqual([
      "/project/settings/devenv"
    ]);
    const devenv = environment.items[0];
    expect(devenv.manualRoute).toBe(true);
    expect(devenv.children?.map(c => c.path)).toEqual([
      "/project/settings/devenv/devices",
      "/project/settings/devenv/executors",
      "/project/settings/runners"
    ]);
  });

  it("撤出主菜单的既有页面经 auxiliary 源保留（严格 Figma 条目口径）", () => {
    const auxPaths = auxiliaryNavItems.map(i => i.path);
    expect(auxPaths).toEqual(
      expect.arrayContaining([
        "/dashboard",
        "/project/design",
        "/project/tasks",
        "/project/analysis",
        "/project/cognition",
        "/project/product-cognition"
      ])
    );
    // auxiliary 项不在任何主菜单组中
    const mainPaths = mainNavGroups.flatMap(g => g.items.map(i => i.path));
    for (const path of auxPaths) {
      expect(mainPaths).not.toContain(path);
    }
    // PLANNED 占位项显式标记（Product Cognition；能力不得伪装已上线）
    const planned = auxiliaryNavItems
      .filter(i => i.planned)
      .map(i => i.path);
    expect(planned).toEqual(["/project/product-cognition"]);
  });

  it("所有导航项的 path 唯一且以 / 开头", () => {
    const paths = flatNavItems.map(i => i.path);
    expect(new Set(paths).size).toBe(paths.length);
    for (const path of paths) {
      expect(path.startsWith("/")).toBe(true);
    }
  });

  it("带子菜单的导航项的子路径均在其路径之下（manualRoute 除外）", () => {
    for (const item of flatNavItems) {
      if (item.manualRoute) {
        // 手工路由条目（开发环境）的子项可跨父前缀
        // （/project/settings/runners 不在 /project/settings/devenv/ 下），
        // 不经 buildRoutes 派生 redirect，不受本契约约束。
        continue;
      }
      for (const child of item.children ?? []) {
        expect(child.path.startsWith(`${item.path}/`)).toBe(true);
      }
    }
  });

  it("精确匹配时激活", () => {
    const item = flatNavItems.find(i => i.path === "/project/requirements")!;
    expect(isNavItemActive(item, "/project/requirements")).toBe(true);
  });

  it("带子菜单的父项在子路径下激活", () => {
    // 主菜单的父子激活由开发环境承载；用合成项验证 isNavItemActive
    // 的父子激活机制本身。
    const item: NavItem = {
      path: "/project/discovery",
      labelKey: "shell.app.side.item.discovery",
      icon: ScanSearch,
      children: [
        { path: "/project/discovery/overview", labelKey: "test" },
        { path: "/project/discovery/branches", labelKey: "test" }
      ]
    };
    expect(isNavItemActive(item, "/project/discovery/overview")).toBe(true);
    expect(isNavItemActive(item, "/project/discovery/branches")).toBe(true);
  });

  it("同前缀的兄弟路径不误激活", () => {
    const requirements = flatNavItems.find(
      i => i.path === "/project/requirements"
    )!;
    expect(isNavItemActive(requirements, "/project/requirements-other")).toBe(
      false
    );
  });

  it("子菜单项按精确路径判定激活", () => {
    const child: NavChild = {
      path: "/project/discovery/overview",
      labelKey: "test"
    };
    expect(isNavChildActive(child, "/project/discovery/overview")).toBe(true);
    expect(isNavChildActive(child, "/project/discovery/branches")).toBe(false);
  });
});
