import {
  createRouter,
  createWebHashHistory,
  createWebHistory,
  type RouteRecordRaw,
  type RouterHistory
} from "vue-router";

import { navGroups, type NavItem } from "./navigation";
import { designRoutes } from "./design";
import PlaceholderView from "@/views/placeholder/PlaceholderView.vue";

/**
 * 应用壳路由表 —— **由导航树派生**（src/router/navigation.ts 是单源）。
 *
 * 派生规则：
 *   - 每个导航项一条路由，path 即导航项 path；
 *   - 带子菜单的导航项，其自身 path 重定向到第一个子项；
 *   - 每个子菜单项一条独立路由（平铺，不做嵌套，保证激活判定简单）；
 *   - 兜底路由渲染 404 占位。
 *
 * 演示期所有业务页面都指向占位视图；业务模块落地时按需替换组件。
 */

/** 导航项 → 路由记录 */
function navItemRoutes(item: NavItem): RouteRecordRaw[] {
  const routes: RouteRecordRaw[] = [];

  if (item.children?.length) {
    // 父项重定向到首个子项（Figma 的 Repository → Overview）
    routes.push({
      path: item.path,
      redirect: item.children[0].path,
      meta: { title: item.labelKey }
    });
    for (const child of item.children) {
      routes.push({
        path: child.path,
        component: PlaceholderView,
        meta: { title: child.labelKey }
      });
    }
    return routes;
  }

  routes.push({
    path: item.path,
    component: PlaceholderView,
    meta: { title: item.labelKey }
  });
  return routes;
}

/** 全部路由记录（导航树 → 路由） */
export function buildRoutes(): RouteRecordRaw[] {
  const routes: RouteRecordRaw[] = [];

  for (const group of navGroups) {
    for (const item of group.items) {
      routes.push(...navItemRoutes(item));
    }
  }

  // 设计系统目录页（无导航外壳的全屏页，meta.layout = "design"）
  routes.push(...designRoutes);

  routes.push({
    path: "/:pathMatch(.*)*",
    name: "not-found",
    component: PlaceholderView,
    meta: { title: "shell.status.notFoundTitle" }
  });

  return routes;
}

/**
 * history 模式：VITE_ROUTER_HISTORY = "hash" | "h5"（默认 h5）。
 * 静态部署在子路径下时用 hash 更稳。
 */
function createHistory(): RouterHistory {
  const mode = import.meta.env.VITE_ROUTER_HISTORY;
  return mode === "hash"
    ? createWebHashHistory()
    : createWebHistory(import.meta.env.BASE_URL);
}

export const router = createRouter({
  history: createHistory(),
  routes: buildRoutes(),
  scrollBehavior: () => ({ top: 0 })
});

export default router;
