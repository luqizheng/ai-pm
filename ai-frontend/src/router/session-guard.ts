import type { RouteLocationNormalizedLoaded, Router } from "vue-router";

import { onUnauthorized } from "@/api/base/session";
import { AUTH_PATHS } from "./auth";

/**
 * 会话彻底失效（401 且刷新不可用）后的统一出口：
 * 清掉本地会话并回到登录页，登录后按 redirect 回跳原页面。
 *
 * 认证页（meta.layout = "auth"）与设计目录（"design"）不跳转：
 * 前者本身就在登录面，后者的 401 属于内容加载失败，不应打断浏览。
 */
export function isSessionExpiryExempt(
  route: Pick<RouteLocationNormalizedLoaded, "meta">
): boolean {
  return route.meta.layout === "auth" || route.meta.layout === "design";
}

/** 回跳地址只接受站内绝对路径，其余一律回默认落点 */
export function buildLoginRedirect(fullPath: string): string | undefined {
  if (!fullPath.startsWith("/") || fullPath.startsWith("//")) return undefined;
  if (fullPath === "/") return undefined;
  return fullPath;
}

export function installSessionExpiryRedirect(router: Router): void {
  onUnauthorized(() => {
    const current = router.currentRoute.value;
    if (isSessionExpiryExempt(current)) return;

    const redirect = buildLoginRedirect(current.fullPath);
    const target = redirect
      ? { path: AUTH_PATHS.login, query: { redirect } }
      : { path: AUTH_PATHS.login };

    void router.replace(target).catch(() => {
      // 并发失效会重复导航到同一目标：忽略重复导航错误即可
    });
  });
}
