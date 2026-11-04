// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import { DESIGN_PATHS, designRoutes } from "./design";
import { buildRoutes } from "./index";

/**
 * 设计系统目录路由契约：
 *   - 15 条 design 路由全部可达且为无壳全屏页（meta.layout = "design"）；
 *   - 路由已挂进 buildRoutes()（App 双外壳判据依赖 meta.layout）；
 *   - 认证/业务路由不受影响（buildRoutes 仍由导航树派生）。
 */

function makeRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
}

describe("design routes", () => {
  it("designRoutes 覆盖 15 条路径", () => {
    expect(designRoutes).toHaveLength(15);
  });

  it("全部路由带 meta.layout=design 且已挂进 buildRoutes", async () => {
    const router = makeRouter();
    for (const route of designRoutes) {
      const resolved = router.resolve(route.path);
      expect(
        resolved.matched.length,
        `unresolved: ${route.path}`
      ).toBeGreaterThan(0);
      expect(resolved.meta.layout).toBe("design");
    }
  });

  it("路径常量与路由一一对应", () => {
    const paths = designRoutes.map(r => r.path);
    expect(paths).toContain(DESIGN_PATHS.index);
    expect(paths).toContain(DESIGN_PATHS.tokens);
    expect(
      Object.values(DESIGN_PATHS.components).every(p => paths.includes(p))
    ).toBe(true);
    expect(
      Object.values(DESIGN_PATHS.navigation).every(p => paths.includes(p))
    ).toBe(true);
    expect(
      Object.values(DESIGN_PATHS.shell).every(p => paths.includes(p))
    ).toBe(true);
  });

  it("业务导航路由不受 design 路由影响", async () => {
    const router = makeRouter();
    const resolved = router.resolve("/dashboard");
    expect(resolved.matched.length).toBeGreaterThan(0);
    expect(resolved.meta.layout).toBeUndefined();
  });

  it("404 兜底仍生效", async () => {
    const router = makeRouter();
    const resolved = router.resolve("/no-such-page");
    expect(resolved.name).toBe("not-found");
  });
});
