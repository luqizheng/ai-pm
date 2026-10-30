import type { RouteRecordRaw } from "vue-router";
import AppShell from "../components/AppShell.vue";
import ComingSoon from "../views/ComingSoon.vue";
import LoginPage from "../views/login/LoginPage.vue";
import NotFound from "../views/NotFound.vue";
import { demoProject } from "../../workspace/fixtures";

/** 静态路由（不需要 DB 菜单即可访问） */
export const staticRoutes: RouteRecordRaw[] = [
  {
    path: "/login",
    name: "login",
    component: LoginPage,
    meta: { public: true, title: "登录" }
  },
  {
    path: "/",
    name: "shell",
    component: AppShell,
    redirect: "/dashboard",
    children: [
      {
        path: "dashboard",
        name: "dashboard",
        component: () => import("../views/dashboard/DashboardView.vue"),
        meta: { title: "概览", icon: "layout-dashboard" }
      },
      {
        path: "profile",
        name: "profile",
        component: () => import("../views/profile/ProfileView.vue"),
        meta: { title: "个人设置", icon: "settings" }
      }
    ]
  },
  // AI 工作空间（P0 竖切）：三栏壳 + 总控台，其余页签落 ComingSoon。
  // 纯新增子树，不触碰 shell 路由与 DB 菜单（spec A9）。
  {
    path: "/workspace/:projectId",
    component: () => import("../../workspace/shell/WorkspaceShell.vue"),
    props: route => ({
      project: demoProject.name,
      page: String(route.meta.title ?? "")
    }),
    redirect: to => `/workspace/${String(to.params.projectId)}/console`,
    children: [
      {
        path: "console",
        name: "workspace-console",
        component: () => import("../../workspace/console/ConsolePage.vue"),
        meta: { title: "项目总控" }
      },
      {
        path: ":page",
        name: "workspace-page",
        component: ComingSoon,
        meta: { title: "建设中" }
      }
    ]
  },
  {
    path: "/:pathMatch(.*)*",
    name: "not-found",
    component: NotFound,
    meta: { title: "页面不存在" }
  }
];
