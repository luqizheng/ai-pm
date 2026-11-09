// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";

/**
 * 应用冷启动回归 —— **首航必须发生在设计目录路由注册之后**。
 *
 * 历史缺陷：设计目录路由过去用 `import("./design")` 异步挂载，而 router.install()
 * 触发的首次导航立刻解析路由表——首航早于注册时 /design 只匹配 404 兜底
 * （无 meta.layout），认证守卫把匿名访客踢去 /auth/login?redirect=/design；且
 * addRoute 不会重新解析已完成的导航（vue-router 只递增 routesVersion），于是
 * 冷启动直达 /design 永远停在登录墙。
 *
 * 这里走真实装配（import("../entry")）而不是手工 push：只有真实装配才覆盖
 * 「entry.ts 等 designRoutesReady 就绪后才装 router」这条时序约定。
 *
 * 设计模块被刻意「慢加载」——浏览器里首航几乎总是早于那次动态 import 落地，
 * 而单测的模块加载顺序恰好相反（后发先至），不制造这个延迟就复现不出缺陷。
 */
describe("应用冷启动（直达 /design）", () => {
  it("匿名直达 /design 停在设计目录，不被守卫踢去登录页", async () => {
    vi.doMock("./design", async () => {
      await new Promise(resolve => setTimeout(resolve, 30));
      return await vi.importActual<typeof import("./design")>("./design");
    });

    // 匿名访客：无任何凭据
    window.localStorage.clear();
    window.sessionStorage.clear();
    // 冷启动地址 —— 必须先于路由模块求值落定：router 在 createWebHistory 时
    // 就把 window.location 定为首航目标，之后再改 URL 已经太晚
    window.history.replaceState({}, "", "/design");
    // 挂载点（entry.ts 挂到 #app）
    const root = document.createElement("div");
    root.id = "app";
    document.body.appendChild(root);

    const { router } = await import("./index");
    await import("../entry");
    await router.isReady();

    expect(router.currentRoute.value.path).toBe("/design");
    expect(router.currentRoute.value.meta.layout).toBe("design");
    expect(router.currentRoute.value.name).toBe("design-index");
  });
});
