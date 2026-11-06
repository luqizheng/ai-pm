import {
  createRouter,
  createWebHashHistory,
  createWebHistory,
  type RouteRecordRaw,
  type RouterHistory
} from "vue-router";

import { navGroups, type NavItem } from "./navigation";
import { AUTH_PATHS, authRoutes } from "./auth";
import { designRoutes } from "./design";
import { getAccessToken, getRefreshToken } from "@/api/base/token-storage";
import NotificationsView from "@/views/notifications/NotificationsView.vue";
import SettingsLandingView from "@/views/settings/SettingsLandingView.vue";
import SettingsProfileView from "@/views/settings/SettingsProfileView.vue";
import SettingsSecurityView from "@/views/settings/SettingsSecurityView.vue";
import SettingsPreferencesView from "@/views/settings/SettingsPreferencesView.vue";
import SettingsNotificationsView from "@/views/settings/SettingsNotificationsView.vue";
import SettingsAiView from "@/views/settings/SettingsAiView.vue";
import SettingsTeamAiView from "@/views/settings/SettingsTeamAiView.vue";
import PlaceholderView from "@/views/placeholder/PlaceholderView.vue";
import ProjectCreateView from "@/views/project/create/ProjectCreateView.vue";
import CognitionPage from "@/views/project/cognition/CognitionPage.vue";
import BaselinePage from "@/views/project/baseline/BaselinePage.vue";
import CommandCenterView from "@/views/project/command-center/CommandCenterView.vue";
import RequirementBaselineView from "@/views/project/requirement-baseline/RequirementBaselineView.vue";
import RequirementDetailView from "@/views/project/requirement-detail/RequirementDetailView.vue";
import RequirementHistoryView from "@/views/project/requirement-history/RequirementHistoryView.vue";
import RequirementImpactView from "@/views/project/requirement-impact/RequirementImpactView.vue";
import RequirementReviewView from "@/views/project/requirement-review/RequirementReviewView.vue";

/**
 * 应用壳路由表 —— **由导航树派生**（src/router/navigation.ts 是单源）。
 *
 * 派生规则：
 *   - 每个导航项一条路由，path 即导航项 path；
 *   - 带子菜单的导航项，其自身 path 重定向到第一个子项；
 *   - 每个子菜单项一条独立路由（平铺，不做嵌套，保证激活判定简单）；
 *   - 认证路由（src/router/auth.ts）单独成组，meta.layout = "auth"，
 *     由 App.vue 切换到认证页外壳（无导航）；
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
        component: child.component ?? PlaceholderView,
        meta: { title: child.labelKey }
      });
    }
    return routes;
  }

  routes.push({
    path: item.path,
    // 导航项自带业务页组件时用它，否则落到占位页
    component: item.component ?? PlaceholderView,
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

  // 认证路由在兜底之前注册，避免被 404 捕获
  routes.push(...authRoutes);

  // 项目创建（S01-02）：动作页，不占主导航——入口 = Personal Home「新建项目」
  // 与顶栏 Project Switcher 的 Create New Project（canCreate）。
  // 创建成功即工作：携带 project_id 直接进入项目总控台（2026-11-05 用户裁决，
  // 原「跳初始化总览」页已删除）。
  // 项目认知（S01-04）：六阶段之 cognition 的下钻页（人工启动 Run + 逐项确认）。
  routes.push({
    path: "/project/cognition",
    component: CognitionPage,
    meta: { title: "s01.cognition.page.title" }
  });

  // 项目基线（S01-05）：提案派生（知识+仓库快照）→ 逐项确认 → 版本不可变。
  routes.push({
    path: "/project/baseline",
    component: BaselinePage,
    meta: { title: "s01.baseline.page.title" }
  });

  routes.push({
    path: "/project/create",
    component: ProjectCreateView,
    meta: { title: "s01.project-create.pageTitle" }
  });

  // 项目总控台（Project Command Center）：项目域工作入口（mock 数据先行），
  // 不占主导航——入口 = 项目切换后的落地页（S01-00 §7.1，该分支合并前先注册直达路由）。
  routes.push({
    path: "/project/command-center",
    component: CommandCenterView,
    meta: { title: "s01.command-center.pageTitle" }
  });

  // 需求评审与确认（S02-UI-03）：人工评审提案并作出正式决策，不占主导航——
  // 入口 = Requirements Workspace 页头「需求评审」。静态段优先于下方动态段匹配。
  routes.push({
    path: "/project/requirements/review",
    component: RequirementReviewView,
    meta: { title: "pm.requirementReview.pageTitle" }
  });

  // 需求基线管理（S02-UI-06）：正式基线快照的管理 / 准备向导 / 比较，
  // 不占主导航——入口 = Requirements Workspace 页头「需求基线」。
  routes.push({
    path: "/project/requirements/baselines",
    component: RequirementBaselineView,
    meta: { title: "pm.requirementBaseline.pageTitle" }
  });

  // 修订与历史（S02-UI-05）：单条需求的修订时间线 / 详情 / 两版比较 / 审计，
  // 不占主导航——入口 = 需求详情页的两处历史入口（页头与 History Tab）。
  routes.push({
    path: "/project/requirements/:requirementId/history",
    component: RequirementHistoryView,
    meta: { title: "pm.requirementHistory.pageTitle" }
  });

  // 变更影响分析（S02-UI-07）：拟议修订的依赖可视化 / 影响面板 / AI 分析 /
  // 人类决策，不占主导航——入口 = 需求详情页页头「影响分析」。
  routes.push({
    path: "/project/requirements/:requirementId/impact",
    component: RequirementImpactView,
    meta: { title: "pm.requirementImpact.pageTitle" }
  });

  // 需求详情与追溯（S02-UI-02）：单条需求检查页，不占主导航——
  // 入口 = Requirements Workspace 详情面板「打开详情页」与追溯链关联需求导航。
  routes.push({
    path: "/project/requirements/:requirementId",
    component: RequirementDetailView,
    meta: { title: "pm.requirementDetail.pageTitle" }
  });

  // 通知中心（Global/Notification 族完整事件面；不在左导航树内，
  // 入口 = 顶栏通知下拉的「查看全部通知」）
  routes.push({
    path: "/notifications",
    component: NotificationsView,
    meta: { title: "global.notification.label" }
  });

  // 设置域（Settings Framework 个人域；不在左导航树内，
  // 入口 = 顶栏账户菜单 UserMenu。无 meta.layout → 应用壳内渲染。
  // 租户/项目域仍在设计目录 /design/settings/*，应用侧暂不落地）
  routes.push(
    {
      path: "/settings",
      redirect: "/settings/personal",
      meta: { title: "settings.personal.landing.title" }
    },
    {
      path: "/settings/personal",
      component: SettingsLandingView,
      meta: { title: "settings.personal.landing.title" }
    },
    {
      path: "/settings/personal/profile",
      component: SettingsProfileView,
      meta: { title: "settings.nav.profile" }
    },
    {
      path: "/settings/personal/security",
      component: SettingsSecurityView,
      meta: { title: "settings.nav.security" }
    },
    {
      path: "/settings/personal/preferences",
      component: SettingsPreferencesView,
      meta: { title: "settings.nav.preferences" }
    },
    {
      path: "/settings/personal/notifications",
      component: SettingsNotificationsView,
      meta: { title: "settings.nav.notifications" }
    },
    {
      path: "/settings/personal/ai",
      component: SettingsAiView,
      meta: { title: "settings.nav.ai" }
    }
  );

  // 团队域设置（0041 Team LLM Provider）：Team≡tenant，AI 配置先落地；
  // 入口 = 设置落地页「团队设置」卡片。
  routes.push({
    path: "/settings/team/ai-providers",
    component: SettingsTeamAiView,
    meta: { title: "settings.nav.aiProviders" }
  });

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

/**
 * 认证守卫：无任何凭据时把业务页重定向到登录页。
 *
 * 只判断凭据是否存在，不判断 access token 是否过期 —— 过期由 HTTP 客户端的
 * 单飞刷新处理；两者都缺失才视为未登录，避免刷新可用时被误踢。
 */
export const PUBLIC_LAYOUTS = new Set(["auth", "bare", "design"]);

router.beforeEach(to => {
  const layout = typeof to.meta.layout === "string" ? to.meta.layout : "";
  if (PUBLIC_LAYOUTS.has(layout)) {
    if (
      to.path === AUTH_PATHS.login &&
      (getAccessToken() || getRefreshToken())
    ) {
      return "/dashboard";
    }
    return true;
  }

  if (!getAccessToken() && !getRefreshToken()) {
    return {
      path: AUTH_PATHS.login,
      query: to.fullPath === "/" ? undefined : { redirect: to.fullPath }
    };
  }

  return true;
});

export default router;
