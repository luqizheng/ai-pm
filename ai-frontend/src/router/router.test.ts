// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import { flatNavItems } from "./navigation";
import { buildRoutes } from "./index";

/**
 * 路由表契约：**由导航树派生**，不允许二者漂移。
 */
describe("router", () => {
  it("每个导航项都有对应路由", () => {
    const routes = buildRoutes();
    const paths = new Set(routes.map(r => r.path));

    for (const item of flatNavItems) {
      expect(paths.has(item.path)).toBe(true);
    }
  });

  it("每个子菜单项都有对应路由", () => {
    const routes = buildRoutes();
    const paths = new Set(routes.map(r => r.path));

    for (const item of flatNavItems) {
      for (const child of item.children ?? []) {
        expect(paths.has(child.path)).toBe(true);
      }
    }
  });

  it("带子菜单的父路由重定向到第一个子项", () => {
    const routes = buildRoutes();
    const repo = routes.find(r => r.path === "/project/repository");
    expect(repo?.redirect).toBe("/project/repository/overview");
  });

  it("存在兜底路由（404）", () => {
    const routes = buildRoutes();
    const notFound = routes.find(r => r.path.includes("pathMatch"));
    expect(notFound).toBeTruthy();
    expect(notFound!.component).toBeTruthy();
  });

  it("路由表无重复路径", () => {
    const paths = buildRoutes().map(r => r.path);
    expect(new Set(paths).size).toBe(paths.length);
  });

  it("导航跳转可解析到路由", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push("/project/execution");
    await router.isReady();

    expect(router.currentRoute.value.path).toBe("/project/execution");
    expect(router.currentRoute.value.meta.title).toBeTruthy();
  });
});
