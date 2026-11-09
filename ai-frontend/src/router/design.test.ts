// @vitest-environment happy-dom
import { beforeAll, describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import { DESIGN_PATHS, designRoutes } from "./design";
import { buildRoutes, designRoutesReady, router } from "./index";

/**
 * 设计系统目录路由契约：
 *   - 47 条 design 路由（16 目录 + 12 AI 交互组件库 + 19 Settings 框架）
 *     全部可达且为无壳全屏页（meta.layout = "design"）；
 *   - 路由已挂进 buildRoutes()（App 双外壳判据依赖 meta.layout）；
 *   - 认证/业务路由不受影响（buildRoutes 仍由导航树派生）；
 *   - 设计目录对**匿名访客开放**：冷启动直达 /design 不得被认证守卫踢去登录页
 *     （回归：设计路由过去是异步注册的，首航早于注册 → /design 落 404 兜底 →
 *     无 meta.layout → 被守卫重定向到 /auth/login?redirect=/design）。
 */

function makeRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
}

describe("design routes", () => {
  it("designRoutes 覆盖 47 条路径（16 目录 + 12 AI 交互 + 19 Settings）", () => {
    expect(designRoutes).toHaveLength(47);
  });

  it("全部路由带 meta.layout=design 且已挂进应用路由", async () => {
    // 设计目录仅开发/单测期按需挂载（发布构建剔除，避免演示数据进 dist）
    await designRoutesReady;
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
    expect(
      Object.values(DESIGN_PATHS.aiInteraction).every(p => paths.includes(p))
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

/**
 * 设计目录的公开性 —— 认证守卫对匿名访客的行为。
 *
 * 用应用真实 router（含守卫）走完整导航，而不是只 resolve：
 * 守卫的判定依据是匹配到的路由的 meta.layout，故必须等设计路由注册就绪。
 */
describe("设计目录对匿名访客开放", () => {
  beforeAll(async () => {
    // 匿名：无任何凭据
    window.localStorage.clear();
    window.sessionStorage.clear();
    await designRoutesReady;
  });

  it("匿名直达 /design 落在目录页而不是登录页", async () => {
    await router.push(DESIGN_PATHS.index);
    await router.isReady();

    expect(router.currentRoute.value.path).toBe(DESIGN_PATHS.index);
    expect(router.currentRoute.value.meta.layout).toBe("design");
    expect(router.currentRoute.value.name).toBe("design-index");
  });

  it("匿名直达设计子页同样放行", async () => {
    await router.push(DESIGN_PATHS.tokens);
    await router.isReady();

    expect(router.currentRoute.value.path).toBe(DESIGN_PATHS.tokens);
    expect(router.currentRoute.value.meta.layout).toBe("design");
  });

  it("未注册的设计子路径落 404 占位，而不是登录墙", async () => {
    // 前缀兜底：设计目录的公开性不依赖路由注册时序
    await router.push("/design/__not-a-real-page__");
    await router.isReady();

    expect(router.currentRoute.value.path).toBe("/design/__not-a-real-page__");
    expect(router.currentRoute.value.name).not.toBe("auth-login");
  });

  it("业务页仍然要求登录（守卫未被放宽）", async () => {
    await router.push("/dashboard");
    await router.isReady();

    expect(router.currentRoute.value.path).toBe("/auth/login");
    expect(router.currentRoute.value.query.redirect).toBe("/dashboard");
  });
});
