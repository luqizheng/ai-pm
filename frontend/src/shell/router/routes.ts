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
    meta: { public: true, title: "shell.routes.login" }
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
        meta: { title: "shell.routes.dashboard", icon: "layout-dashboard" }
      },
      {
        path: "profile",
        name: "profile",
        component: () => import("../views/profile/ProfileView.vue"),
        meta: { title: "shell.routes.profile", icon: "settings" }
      },
      {
        // 项目 ↔ 代码仓库关系（P0 CRUD）：静态挂 shell，不依赖 DB 菜单 publish。
        path: "projects/:projectId/repositories",
        name: "project-repositories",
        component: () => import("../views/projects/repositories/index.vue"),
        meta: { title: "shell.routes.projectRepositories" }
      },
      {
        // 项目总控台（S01 · I5-D）：六区块聚合视图，静态挂 shell。
        path: "projects/:projectId/command-center",
        name: "project-command-center",
        component: () => import("../views/pm/command-center/index.vue"),
        meta: { title: "shell.routes.projectCommandCenter" }
      },
      {
        // 冷启动导入向导：静态路由，不依赖 DB 菜单。
        path: "onboarding/import",
        name: "project-import",
        component: () => import("../views/onboarding/ProjectImportWizard.vue"),
        meta: { title: "shell.routes.projectImport" }
      },
      {
        // 客户主数据（0017）：静态路由，不依赖 DB 菜单。
        path: "customers",
        name: "customers",
        component: () => import("../views/pm/customers/index.vue"),
        meta: { title: "shell.routes.customers" }
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
        meta: { title: "shell.routes.workspaceConsole" }
      },
      {
        path: "team",
        name: "workspace-team",
        component: () => import("../../workspace/pages/TeamPage.vue"),
        meta: { title: "shell.routes.workspaceTeam" }
      },
      {
        path: "execution",
        name: "workspace-execution",
        component: () => import("../../workspace/pages/ExecutionPage.vue"),
        meta: { title: "shell.routes.workspaceExecution" }
      },
      {
        path: "goals",
        name: "workspace-goals",
        component: () => import("../../workspace/pages/GoalsPage.vue"),
        meta: { title: "shell.routes.workspaceGoals" }
      },
      {
        path: "models",
        name: "workspace-models",
        component: () => import("../../workspace/pages/ModelsPage.vue"),
        meta: { title: "shell.routes.workspaceModels" }
      },
      {
        path: "activity",
        name: "workspace-activity",
        component: () => import("../../workspace/pages/ActivityPage.vue"),
        meta: { title: "shell.routes.workspaceActivity" }
      },
      {
        path: "requirements",
        name: "workspace-requirements",
        component: () => import("../../workspace/pages/RequirementsPage.vue"),
        meta: { title: "shell.routes.workspaceRequirements" }
      },
      {
        path: "decisions",
        name: "workspace-decisions",
        component: () => import("../../workspace/pages/DecisionsPage.vue"),
        meta: { title: "shell.routes.workspaceDecisions" }
      },
      {
        path: "knowledge",
        name: "workspace-knowledge",
        component: () => import("../../workspace/pages/KnowledgePage.vue"),
        meta: { title: "shell.routes.workspaceKnowledge" }
      },
      {
        path: "artifacts",
        name: "workspace-artifacts",
        component: () => import("../../workspace/pages/ArtifactsPage.vue"),
        meta: { title: "shell.routes.workspaceArtifacts" }
      },
      {
        path: "delivery",
        name: "workspace-delivery",
        component: () => import("../../workspace/pages/DeliveryPage.vue"),
        meta: { title: "shell.routes.workspaceDelivery" }
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
        meta: { title: "shell.routes.classicRequirements" }
      },
      {
        path: "classic/tasks",
        name: "workspace-classic-tasks",
        component: () => import("../../shell/views/pm/tasks/index.vue"),
        meta: { title: "shell.routes.classicTasks" }
      },
      {
        path: "classic/approvals",
        name: "workspace-classic-approvals",
        component: () => import("../../shell/views/pm/approvals/index.vue"),
        meta: { title: "shell.routes.classicApprovals" }
      },
      {
        path: ":page",
        name: "workspace-page",
        component: ComingSoon,
        meta: { title: "shell.routes.comingSoon" }
      }
    ]
  },
  {
    path: "/:pathMatch(.*)*",
    name: "not-found",
    component: NotFound,
    meta: { title: "shell.routes.notFound" }
  }
];
