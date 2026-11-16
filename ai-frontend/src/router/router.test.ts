// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import { flatNavItems } from "./navigation";
import { AUTH_PATHS } from "./auth";
import { buildRoutes } from "./index";
import PlaceholderView from "@/views/placeholder/PlaceholderView.vue";

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

  it("项目设置根路径重定向到 General", () => {
    const routes = buildRoutes();
    const settings = routes.find(r => r.path === "/project/settings");
    expect(settings?.redirect).toBe("/project/settings/general");
  });

  it("旧仓库 / 执行环境路径重定向到 Project Setting 对应节", () => {
    const routes = buildRoutes();
    const redirects = new Map(
      routes
        .filter(r => typeof r.redirect === "string")
        .map(r => [r.path, r.redirect])
    );
    expect(redirects.get("/project/repository")).toBe(
      "/project/settings/repositories"
    );
    expect(redirects.get("/project/repository/overview")).toBe(
      "/project/settings/repositories"
    );
    expect(redirects.get("/project/environments")).toBe(
      "/project/settings/runners"
    );
    // 详情路径函数式重定向保 deviceId 参数
    const detail = routes.find(
      r => r.path === "/project/environments/:deviceId"
    );
    expect(typeof detail?.redirect).toBe("function");
  });

  it("Project Setting 五节路由已注册", () => {
    const routes = buildRoutes();
    const paths = new Set(routes.map(r => r.path));
    for (const path of [
      "/project/settings/general",
      "/project/settings/members",
      "/project/settings/repositories",
      "/project/settings/runners",
      "/project/settings/runners/:deviceId",
      "/project/settings/ai"
    ]) {
      expect(paths.has(path), `${path} 未注册`).toBe(true);
    }
  });

  it("C3：Analysis / Product Cognition 占位路由渲染占位组件", () => {
    const routes = buildRoutes();
    for (const path of ["/project/analysis", "/project/product-cognition"]) {
      const route = routes.find(r => r.path === path);
      expect(route, `${path} 未注册`).toBeTruthy();
      // 占位路由 = 无业务组件，落 PlaceholderView（能力不得伪装已上线）
      expect(route!.component).toBe(PlaceholderView);
    }
  });

  it("C3：项目认知与项目总控台由导航派生为真组件路由", () => {
    const routes = buildRoutes();
    for (const path of ["/project/cognition", "/project/command-center"]) {
      const route = routes.find(r => r.path === path);
      expect(route, `${path} 未注册`).toBeTruthy();
      expect(route!.component).not.toBe(PlaceholderView);
    }
  });

  it("C3：开发环境父子路由（父 redirect / Devices 真页 / Executors 占位 / 旧路径兼容）", () => {
    const routes = buildRoutes();
    const paths = new Set(routes.map(r => r.path));

    // 父路由重定向到首个子项 Devices
    const parent = routes.find(r => r.path === "/project/settings/devenv");
    expect(parent?.redirect).toBe("/project/settings/devenv/devices");

    // Devices 仍由 BootstrapView 承载；Executors 为 PLANNED 占位
    const devices = routes.find(
      r => r.path === "/project/settings/devenv/devices"
    );
    expect(devices).toBeTruthy();
    expect(devices!.component).not.toBe(PlaceholderView);
    const executors = routes.find(
      r => r.path === "/project/settings/devenv/executors"
    );
    expect(executors!.component).toBe(PlaceholderView);

    // 旧路径 /project/devenv 兼容：函数式重定向（保留 query）
    const legacy = routes.find(r => r.path === "/project/devenv");
    expect(typeof legacy?.redirect).toBe("function");

    // Runners 与详情深链保持注册
    expect(paths.has("/project/settings/runners")).toBe(true);
    expect(paths.has("/project/settings/runners/:deviceId")).toBe(true);
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

  it("认证路由已注册且不落在兜底之前被捕获（meta.layout = auth/bare）", () => {
    const routes = buildRoutes();
    const notFoundIndex = routes.findIndex(r => r.path.includes("pathMatch"));

    for (const path of Object.values(AUTH_PATHS)) {
      const index = routes.findIndex(r => r.path === path);
      expect(index, `认证路由 ${path} 未注册`).toBeGreaterThanOrEqual(0);
      expect(index, `认证路由 ${path} 被兜底路由遮蔽`).toBeLessThan(
        notFoundIndex
      );
    }

    for (const route of routes.filter(r =>
      Object.values(AUTH_PATHS).includes(r.path as never)
    )) {
      expect(["auth", "bare"]).toContain(route.meta?.layout);
    }
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
