import { describe, expect, it } from "vitest";
import { ScanSearch } from "lucide-vue-next";

import {
  flatNavItems,
  isNavChildActive,
  isNavItemActive,
  navGroups,
  type NavChild,
  type NavItem
} from "./navigation";

/**
 * 导航树契约（壳与路由的单源）。
 */
describe("navigation", () => {
  it("四个导航分组按个人 / 工作区 / 项目模块 / 管理排列", () => {
    expect(navGroups.map(g => g.id)).toEqual([
      "personal",
      "workspace",
      "modules",
      "admin"
    ]);
  });

  it("只有项目模块分组标记为项目作用域", () => {
    expect(navGroups.filter(g => g.projectScoped).map(g => g.id)).toEqual([
      "modules"
    ]);
  });

  it("所有导航项的 path 唯一且以 / 开头", () => {
    const paths = flatNavItems.map(i => i.path);
    expect(new Set(paths).size).toBe(paths.length);
    for (const path of paths) {
      expect(path.startsWith("/")).toBe(true);
    }
  });

  it("带子菜单的导航项的子路径均在其路径之下", () => {
    for (const item of flatNavItems) {
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
    // 当前导航树暂无带子菜单的项（仓库已迁入 Project Setting），
    // 用合成项验证 isNavItemActive 的父子激活机制。
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
