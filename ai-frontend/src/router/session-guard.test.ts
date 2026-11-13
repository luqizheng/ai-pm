// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

const Blank = { render: () => null };

async function setup(initial: string) {
  const { installSessionExpiryRedirect } = await import("./session-guard");
  const { notifyUnauthorized } = await import("@/api/base/session");

  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/dashboard", component: Blank },
      {
        path: "/design",
        component: Blank,
        meta: { layout: "design" }
      },
      {
        path: "/auth/login",
        component: Blank,
        meta: { layout: "auth" }
      }
    ]
  });

  installSessionExpiryRedirect(router);
  await router.push(initial);
  await router.isReady();

  notifyUnauthorized();
  await new Promise(resolve => setTimeout(resolve, 0));
  return router;
}

describe("installSessionExpiryRedirect", () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it("业务页会话失效跳登录页并保留回跳地址", async () => {
    const router = await setup("/dashboard");
    expect(router.currentRoute.value.path).toBe("/auth/login");
    expect(router.currentRoute.value.query.redirect).toBe("/dashboard");
  });

  it("已在登录页时不重复跳转", async () => {
    const router = await setup("/auth/login");
    expect(router.currentRoute.value.path).toBe("/auth/login");
    expect(router.currentRoute.value.query.redirect).toBeUndefined();
  });

  it("设计目录不被打断", async () => {
    const router = await setup("/design");
    expect(router.currentRoute.value.path).toBe("/design");
  });
});
