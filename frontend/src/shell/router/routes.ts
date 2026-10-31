import type { RouteRecordRaw } from "vue-router";
import AppShell from "../components/AppShell.vue";
import ComingSoon from "../views/ComingSoon.vue";
import LoginPage from "../views/login/LoginPage.vue";
import NotFound from "../views/NotFound.vue";
import { demoProject, demoProjectId } from "../../workspace/fixtures";

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
      // 终审 M3：非演示项目降级时面包屑显示真实参数，不冒充演示项目名
      project:
        String(route.params.projectId) === demoProjectId
          ? demoProject.name
          : String(route.params.projectId),
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
        path: "team",
        name: "workspace-team",
        component: () => import("../../workspace/pages/TeamPage.vue"),
        meta: { title: "AI 团队" }
      },
      {
        path: "execution",
        name: "workspace-execution",
        component: () => import("../../workspace/pages/ExecutionPage.vue"),
        meta: { title: "AI 执行中心" }
      },
      {
        path: "goals",
        name: "workspace-goals",
        component: () => import("../../workspace/pages/GoalsPage.vue"),
        meta: { title: "目标计划" }
      },
      {
        path: "activity",
        name: "workspace-activity",
        component: () => import("../../workspace/pages/ActivityPage.vue"),
        meta: { title: "项目动态" }
      },
      {
        path: "requirements",
        name: "workspace-requirements",
        component: () => import("../../workspace/pages/RequirementsPage.vue"),
        meta: { title: "需求管理" }
      },
      {
        path: "decisions",
        name: "workspace-decisions",
        component: () => import("../../workspace/pages/DecisionsPage.vue"),
        meta: { title: "决策中心" }
      },
      {
        path: "knowledge",
        name: "workspace-knowledge",
        component: () => import("../../workspace/pages/KnowledgePage.vue"),
        meta: { title: "项目知识" }
      },
      {
        path: "artifacts",
        name: "workspace-artifacts",
        component: () => import("../../workspace/pages/ArtifactsPage.vue"),
        meta: { title: "产物验收" }
      },
      {
        path: "focus/:objectType/:id",
        name: "workspace-focus",
        component: () => import("../../workspace/focus/FocusView.vue"),
        meta: { title: "" }
      },
      {
        path: "classic/overview",
        redirect: to => `/workspace/${String(to.params.projectId)}/console`
      },
      {
        path: "classic/requirements",
        name: "workspace-classic-requirements",
        component: () => import("../../shell/views/pm/requirements/index.vue"),
        meta: { title: "需求管理" } // i18n-ignore: route meta 直接作为页签标题
      },
      {
        path: "classic/tasks",
        name: "workspace-classic-tasks",
        component: () => import("../../shell/views/pm/tasks/index.vue"),
        meta: { title: "任务" } // i18n-ignore: route meta 直接作为页签标题
      },
      {
        path: "classic/approvals",
        name: "workspace-classic-approvals",
        component: () => import("../../shell/views/pm/approvals/index.vue"),
        meta: { title: "审批确认" } // i18n-ignore: route meta 直接作为页签标题
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
