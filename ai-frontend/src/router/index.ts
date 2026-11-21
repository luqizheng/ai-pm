import {
  createRouter,
  createWebHashHistory,
  createWebHistory,
  type RouteRecordRaw,
  type RouterHistory
} from "vue-router";

import { globalNavConfig, projectNavConfig, type NavItem } from "./navigation";
import { AUTH_PATHS, authRoutes } from "./auth";
import { getAccessToken, getRefreshToken } from "@/api/base/token-storage";
import SettingsLandingView from "@/views/settings/SettingsLandingView.vue";
import SettingsProfileView from "@/views/settings/SettingsProfileView.vue";
import SettingsSecurityView from "@/views/settings/SettingsSecurityView.vue";
import SettingsPreferencesView from "@/views/settings/SettingsPreferencesView.vue";
import SettingsNotificationsView from "@/views/settings/SettingsNotificationsView.vue";
import PlaceholderView from "@/views/placeholder/PlaceholderView.vue";
import ProjectCreateView from "@/views/project/create/ProjectCreateView.vue";
import BaselinePage from "@/views/project/baseline/BaselinePage.vue";
import TasksBacklogView from "@/views/project/tasks/backlog/TasksBacklogView.vue";
import SettingsProjectPlaceholderView from "@/views/project/settings/SettingsProjectPlaceholderView.vue";
import SettingsProjectRepositoriesView from "@/views/project/settings/SettingsProjectRepositoriesView.vue";
import SettingsProjectSshKeysView from "@/views/project/settings/SettingsProjectSshKeysView.vue";
import SettingsProjectRunnersView from "@/views/project/settings/SettingsProjectRunnersView.vue";
import SettingsProjectRunnerDetailView from "@/views/project/settings/SettingsProjectRunnerDetailView.vue";
import SettingsProjectAiRolesView from "@/views/project/settings/ai/SettingsProjectAiRolesView.vue";
import SettingsProjectAiOverviewView from "@/views/project/settings/ai/SettingsProjectAiOverviewView.vue";
import SettingsProjectAiModelPoliciesView from "@/views/project/settings/ai/SettingsProjectAiModelPoliciesView.vue";
import SettingsProjectAiMonitorView from "@/views/project/settings/ai/SettingsProjectAiMonitorView.vue";
import SettingsProjectAiExecutionPoliciesView from "@/views/project/settings/ai/SettingsProjectAiExecutionPoliciesView.vue";
import RequirementDetailView from "@/views/project/requirement-detail/RequirementDetailView.vue";
import RequirementHistoryView from "@/views/project/requirement-history/RequirementHistoryView.vue";
import OnboardingWelcomeView from "@/views/onboarding/OnboardingWelcomeView.vue";
import OnboardingCreateTeamView from "@/views/onboarding/OnboardingCreateTeamView.vue";
import OnboardingInviteMembersView from "@/views/onboarding/OnboardingInviteMembersView.vue";
import OnboardingInvitationLinkView from "@/views/onboarding/OnboardingInvitationLinkView.vue";
import TeamInvitationAcceptView from "@/views/team/TeamInvitationAcceptView.vue";

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

/** 全部路由记录（导航树 → 路由；多配置源派生，见 navigation.ts 三源说明） */
export function buildRoutes(): RouteRecordRaw[] {
  const routes: RouteRecordRaw[] = [];

  // 导航派生路由：全局 + 项目两源。settingsNavConfig 仅供侧栏渲染（C3 接线），
  // 其路由由下方手工段注册，不在此派生——避免重复注册触发重复路径守卫。
  for (const group of [...globalNavConfig, ...projectNavConfig]) {
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
  // 项目认知（/project/cognition）已迁入导航树（projectNavConfig，C3），
  // 路由由导航配置派生；项目基线（/project/baseline）保持手工注册——
  // 入口 = 总控台 Baseline 卡片（NAV-0202，Phase 2 接线）。
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

  // AI 会话完整页：入口已合并进顶栏「助手」按钮（AI Chat 常驻列），主导航
  // 不再单列；路由保留供深链——use-shell-chrome 以本路径判定「助手列收起、
  // 上下文栏保留」的三列特例（ASSISTANT_PATH），P0-008a 亦依赖该深链。
  routes.push({
    path: "/project/assistant",
    component: () => import("@/views/project/assistant/AssistantView.vue"),
    meta: { title: "shell.app.side.item.assistant" }
  });

  // 项目总控台（/project/command-center）已迁入导航树（projectHomeGroup，
  // Current Project 区域置顶单条，C3）；其 mock-preview 分支仍受
  // release-guard 约束（AGENTS 规则 4），不得视作已完成业务能力。

  // ─── 执行与协作（S05 · docs/scenarios/s05/ui，2026-11-10）───
  // 七页全部接真后端（task-runs / waiting / execution-overview / run-events /
  // collaboration / 各 Run 附属事实），无 mock 分支。
  //
  // `/project/execution` 本身由导航树派生（navigation.ts 已登记组件），
  // 此处只补不占导航的子页——重复注册会触发「路由表无重复路径」守卫。
  routes.push(
    {
      path: "/project/execution/runs/:runId",
      component: () => import("@/views/project/execution/RunDetailView.vue"),
      meta: { title: "s05.execution.page.runDetailTitle" }
    },
    {
      path: "/project/execution/collaboration/:taskId",
      component: () =>
        import("@/views/project/execution/CollaborationView.vue"),
      meta: { title: "s05.execution.page.collaborationTitle" }
    },
    {
      path: "/project/execution/intervention",
      component: () =>
        import("@/views/project/execution/HumanInterventionView.vue"),
      meta: { title: "s05.execution.page.interventionTitle" }
    },
    {
      path: "/project/execution/authorization",
      component: () =>
        import("@/views/project/execution/ExecutionAuthorizationView.vue"),
      meta: { title: "s05.execution.page.authorizationTitle" }
    },
    {
      path: "/project/execution/review",
      component: () =>
        import("@/views/project/execution/DeliveryReviewView.vue"),
      meta: { title: "s05.execution.page.reviewTitle" }
    },
    {
      path: "/project/execution/history",
      component: () =>
        import("@/views/project/execution/ExecutionHistoryView.vue"),
      meta: { title: "s05.execution.page.historyTitle" }
    }
  );

  // ─── 设计与技术决策（S03 · docs/ui/s03，2026-11-10）───
  // 四个子页 = 独立路由 + 页内页签条（DesignFrame，同 TasksFrame 组织）。
  // 全部接真后端（/v1/pm/projects/{id}/designs · adrs · design-analysis ·
  // design-approvals · design-workspace · implementation-readiness），
  // 无 mock 分支：发布构建与 dev 走同一条真数据路径。
  routes.push(
    {
      path: "/project/design/technical-designs",
      component: () =>
        import("@/views/project/design/designs/TechnicalDesignsView.vue"),
      meta: { title: "s03.design.page.designsTitle" }
    },
    {
      path: "/project/design/technical-designs/:designId",
      component: () =>
        import("@/views/project/design/designs/TechnicalDesignDetailView.vue"),
      meta: { title: "s03.design.page.designDetailTitle" }
    },
    {
      path: "/project/design/adrs",
      component: () => import("@/views/project/design/adrs/AdrView.vue"),
      meta: { title: "s03.design.page.adrsTitle" }
    },
    {
      path: "/project/design/adrs/:adrId",
      component: () => import("@/views/project/design/adrs/AdrDetailView.vue"),
      meta: { title: "s03.design.page.adrDetailTitle" }
    },
    {
      path: "/project/design/readiness",
      component: () =>
        import(
          "@/views/project/design/readiness/ImplementationReadinessView.vue"
        ),
      meta: { title: "s03.design.page.readinessTitle" }
    }
  );

  // ─── 任务管理（S04 · docs/scenarios/s04/ui，2026-11-10）───
  // 子页为独立路由 + 页内页签条（TasksFrame，同 AiTeamFrame 组织）。
  // 全部子页接真后端：总览/任务池（/v1/pm/tasks 系）、拆解（task-plans）、
  // 执行规划依赖图/时间线（dependency-graph）、执行准备（execution-readiness）、
  // 任务详情（GET /v1/pm/tasks/{task_id}）。（C5 注释订正：原「mock 预览」
  // 描述为过期口径。）
  routes.push(
    {
      // 执行规划含依赖图/时间线两个子视图，/planning 重定向到依赖图
      path: "/project/tasks/planning",
      redirect: "/project/tasks/planning/graph",
      meta: { title: "s04.tasks.page.planningTitle" }
    },
    {
      // 任务拆解（S04-UI-02）：接真后端 task-plans 系。
      path: "/project/tasks/breakdown",
      component: () =>
        import("@/views/project/tasks/breakdown/TaskBreakdownView.vue"),
      meta: { title: "s04.tasks.page.breakdownTitle" }
    },
    {
      // 依赖图（S04-UI-05）：接真后端 dependency-graph。
      path: "/project/tasks/planning/graph",
      component: () =>
        import("@/views/project/tasks/planning/DependencyGraphView.vue"),
      meta: { title: "s04.tasks.page.planningTitle" }
    },
    {
      // 时间线（S04-UI-06）：接真后端 dependency-graph + 排期列。
      path: "/project/tasks/planning/timeline",
      component: () =>
        import("@/views/project/tasks/planning/TimelineView.vue"),
      meta: { title: "s04.tasks.page.planningTitle" }
    },
    {
      // 执行准备度（S04-UI-07）：接真后端 execution-readiness。
      path: "/project/tasks/readiness",
      component: () =>
        import("@/views/project/tasks/readiness/ExecutionReadinessView.vue"),
      meta: { title: "s04.tasks.page.readinessTitle" }
    },
    {
      // 任务详情（S04-UI-03）：接真后端 GET /v1/pm/tasks/{task_id}。
      path: "/project/tasks/:taskId",
      component: () =>
        import("@/views/project/tasks/detail/TaskDetailView.vue"),
      meta: { title: "s04.tasks.page.detailTitle" }
    },
    {
      path: "/project/tasks/backlog",
      component: TasksBacklogView,
      meta: { title: "s04.tasks.page.backlogTitle" }
    }
  );

  // ─── Analysis 报告详情（Phase 2 P2-01）───
  // 列表页 /project/analysis 由导航树派生（navigation.ts 单源）；详情页不占
  // 导航，手工注册。组件随 P2-03 充实为六节结构（裁决 D-2）。
  routes.push({
    path: "/project/analysis/reports/:reportId",
    component: () =>
      import("@/views/project/analysis/AnalysisReportDetailView.vue"),
    meta: { title: "analysis.detail.title" }
  });

  // Project Setting（V1.0 稿，2026-11-07）：项目配置中心，不占主导航——
  // 入口 = 项目总控台页头「项目设置」（CommandCenterHeader settings 事件）。
  // Git Repositories / Runners 两节由原独立页面迁入（真数据）；General /
  // Members / AI Configuration 三节规划中（占位页 + 侧栏 proposed 徽标）。
  routes.push(
    {
      path: "/project/settings",
      redirect: "/project/settings/general",
      meta: { title: "settings.project.scopeLabel" }
    },
    {
      path: "/project/settings/general",
      component: SettingsProjectPlaceholderView,
      meta: { title: "settings.nav.projectGeneral" }
    },
    {
      path: "/project/settings/members",
      component: SettingsProjectPlaceholderView,
      meta: { title: "settings.nav.projectMembers" }
    },
    {
      path: "/project/settings/repositories",
      component: SettingsProjectRepositoriesView,
      meta: { title: "s01.repositories.page.title" }
    },
    {
      path: "/project/settings/ssh-keys",
      component: SettingsProjectSshKeysView,
      meta: { title: "s01.ssh-keys.page.title" }
    },
    {
      path: "/project/settings/runners",
      component: SettingsProjectRunnersView,
      meta: { title: "environments.page.title" }
    },
    {
      path: "/project/settings/runners/:deviceId",
      component: SettingsProjectRunnerDetailView,
      meta: { title: "environments.detail.title" }
    },
    // AI Team（UI-1/UI-2/UI-3，U-2=A 五子页全部落地）：外层仍是侧栏单条
    // 「AI 配置」入口，五子页走子路由 + 页内页签条（AiTeamFrame）。
    // UI-1 落地 Roles 与 Model Policies；UI-2 落地 Overview 与 Execution
    // Monitor（U-1=C 聚合端点）；UI-3 落地 Execution Policies（四层策略读写，
    // org/task_type 层 V1 只读）。
    // 各子页 meta.title 保持 settings.nav.projectAi（页头标题同源）。
    {
      path: "/project/settings/ai",
      redirect: "/project/settings/ai/roles",
      meta: { title: "settings.nav.projectAi" }
    },
    {
      path: "/project/settings/ai/roles",
      component: SettingsProjectAiRolesView,
      meta: { title: "settings.nav.projectAi" }
    },
    {
      path: "/project/settings/ai/overview",
      component: SettingsProjectAiOverviewView,
      meta: { title: "settings.nav.projectAi" }
    },
    {
      path: "/project/settings/ai/model-policies",
      component: SettingsProjectAiModelPoliciesView,
      meta: { title: "settings.nav.projectAi" }
    },
    {
      path: "/project/settings/ai/monitor",
      component: SettingsProjectAiMonitorView,
      meta: { title: "settings.nav.projectAi" }
    },
    {
      path: "/project/settings/ai/execution-policies",
      component: SettingsProjectAiExecutionPoliciesView,
      meta: { title: "settings.nav.projectAi" }
    }
  );

  // 开发环境（R-1/P-01：Devices / Executors / Runners 三分，挂项目设置 → 开发环境）。
  // Devices 沿用 BootstrapView（真设备 API 六端点）；Executors 为 PLANNED
  // 占位（不得用 Devices 页伪装 Executor 管理，NAV-0303）。旧路径
  // /project/devenv 带 query 重定向到 Devices 新路径——兼容清单 C-7 的
  // redirect 部分提前落地（受导航 children 前缀契约驱动），BootstrapView
  // 页面自身的迁移仍属 Phase 3。
  routes.push(
    {
      path: "/project/settings/devenv",
      redirect: "/project/settings/devenv/devices",
      meta: { title: "shell.app.side.item.devenv" }
    },
    {
      path: "/project/settings/devenv/devices",
      component: () => import("@/views/project/devenv/BootstrapView.vue"),
      meta: { title: "shell.app.side.item.devenvDevices" }
    },
    {
      path: "/project/settings/devenv/executors",
      component: PlaceholderView,
      meta: { title: "shell.app.side.item.devenvExecutors" }
    }
  );

  // 旧独立入口重定向（Git 仓库 / Runners / 开发环境迁入 Project Setting 统一入口）。
  routes.push(
    {
      path: "/project/repository",
      redirect: "/project/settings/repositories"
    },
    {
      path: "/project/repository/overview",
      redirect: "/project/settings/repositories"
    },
    {
      path: "/project/repository/branches",
      redirect: "/project/settings/repositories"
    },
    {
      path: "/project/environments",
      redirect: "/project/settings/runners"
    },
    {
      path: "/project/environments/:deviceId",
      redirect: to => ({
        path: `/project/settings/runners/${String(to.params.deviceId)}`
      })
    },
    {
      path: "/project/devenv",
      redirect: to => ({
        path: "/project/settings/devenv/devices",
        query: to.query
      })
    }
  );

  // 团队成员管理迁移重定向（C4：/admin/members → 团队设置子树成员与邀请）。
  routes.push({
    path: "/admin/members",
    redirect: "/settings/team/members"
  });

  // Team Owner 继任（P4-ORG UX 修订 V1.0 / Figma T3）：成员页发起的三步
  // 流程页，不占主导航——入口 = 成员页「变更 Team Owner」。助手可见性随
  // /settings/team/members 前缀规则覆盖（navigation.ts ASSISTANT_HIDDEN_RULES）。
  routes.push({
    path: "/settings/team/members/owner-succession",
    component: () =>
      import("@/views/settings/team/TeamOwnerSuccessionView.vue"),
    meta: { title: "team.succession.title" }
  });

  // 收件箱占位退役（Q-10/C5）：旧 /inbox 统一跳通知中心。
  routes.push({
    path: "/inbox",
    redirect: "/notifications"
  });

  // 需求评审与确认（S02-UI-03）：人工评审提案并作出正式决策，不占主导航——
  // 入口 = Requirements Workspace 页头「需求评审」。静态段优先于下方动态段匹配。
  // 需求评审（S02-UI-03）：接真后端 requirement-review-queue（待确认修订）。
  routes.push({
    path: "/project/requirements/review",
    component: () =>
      import("@/views/project/requirement-review/RequirementReviewView.vue"),
    meta: { title: "pm.requirementReview.pageTitle" }
  });

  // 需求基线管理（S02-UI-06）：正式基线快照的管理 / 准备向导 / 比较，
  // 不占主导航——入口 = Requirements Workspace 页头「需求基线」。
  // 需求基线（S02-UI-04）：接真后端 requirement-baseline（派生视图）。
  routes.push({
    path: "/project/requirements/baselines",
    component: () =>
      import(
        "@/views/project/requirement-baseline/RequirementBaselineView.vue"
      ),
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
  // 变更影响分析（S02-UI-07）：接真后端 requirements/{id}/impact。
  routes.push({
    path: "/project/requirements/:requirementId/impact",
    component: () =>
      import("@/views/project/requirement-impact/RequirementImpactView.vue"),
    meta: { title: "pm.requirementImpact.pageTitle" }
  });

  // 图形产物工作区（V0-8）：槽位列表 + 产物查看 + 表格视图 + 溯源。
  // 不占主导航——入口 = 右栏 Graph 页签与对话卡。
  routes.push({
    path: "/project/visualizations",
    component: () =>
      import("@/views/project/visualizations/VisualizationsView.vue"),
    meta: { title: "viz.tab.title" }
  });

  // 需求详情与追溯（S02-UI-02）：单条需求检查页，不占主导航——
  // 入口 = Requirements Workspace 详情面板「打开详情页」与追溯链关联需求导航。
  routes.push({
    path: "/project/requirements/:requirementId",
    component: RequirementDetailView,
    meta: { title: "pm.requirementDetail.pageTitle" }
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
    }
  );

  // 团队设置（V1.2 Team 子树父项）：/settings/team 由导航子树派生为
  // redirect 首子项「团队常规」（navItemRoutes children 规则）。旧落地页
  // SettingsTeamLandingView 随子树化退役；账户菜单 team-settings 去向不变，
  // 经重定向可达。

  // 个人域「AI 偏好」（/settings/personal/ai）已随同一裁决移除；
  // AI 配置（模型服务）= 团队域独立页面 /settings/team/ai-providers。

  // 团队域设置（0041 Team LLM Provider）的 /settings/team/ai-providers 已迁入
  // 导航树（navigation.ts admin 组），路由由 buildRoutes 派生——保持单源。

  // ─── Team Onboarding（0044，产品裁决 Q1：首登必须建团或入团）───
  // 无 Skip/Later；layout="bare"（应用壳之外）但**不进 PUBLIC_LAYOUTS**——
  // 接受邀请/认领建团都必须来自认证上下文，路由守卫仍强制登录。
  routes.push(
    {
      path: "/onboarding",
      component: OnboardingWelcomeView,
      meta: { layout: "bare", title: "team.onboarding.welcome.title" }
    },
    {
      path: "/onboarding/create",
      component: OnboardingCreateTeamView,
      meta: { layout: "bare", title: "team.onboarding.create.title" }
    },
    {
      path: "/onboarding/invite",
      component: OnboardingInviteMembersView,
      meta: { layout: "bare", title: "team.onboarding.invite.title" }
    },
    {
      path: "/onboarding/link",
      component: OnboardingInvitationLinkView,
      meta: { layout: "bare", title: "team.onboarding.link.title" }
    }
  );

  // 邀请接受页（邮件邀请与邀请链接同一接受面；未登录带 redirect 回跳）。
  routes.push({
    path: "/invitation/accept",
    component: TeamInvitationAcceptView,
    meta: { layout: "auth", title: "team.accept.title" }
  });

  // 团队常规（/settings/team/general）、成员与邀请（/settings/team/members）、
  // AI Provider（/settings/team/ai-providers）、Git 凭证占位
  // （/settings/team/credentials）均由导航子树派生（workspaceGroup Team
  // 设置子项，C4）——不再手工注册，避免重复路径。

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
 * 设计系统目录路由（无导航外壳的全屏页，meta.layout = "design"）的注册就绪 promise。
 *
 * 全部由设计稿样本数据驱动，仅开发期按需挂载；发布构建里该分支被静态剔除，
 * 目录页与其 fixtures 不会进入 dist（见 build/release-guard.ts）。
 * —— 因此这里必须用**动态 import**：静态 import 会把 design.ts 拉进模块图，
 * 其中的懒加载视图 import 带着 fixtures 一起进 dist（摇树救不了 import() 的
 * 副作用，release-guard 会当场拦下发布构建）。
 *
 * 注册必须**早于 router 的首次导航**：router.install() 触发的首航会**同步**解析
 * 路由表，而 import("./design") 要等一次模块加载才落地——首航早于注册时
 * /design 只匹配 404 兜底（无 meta.layout），认证守卫会把匿名访客踢去登录页；
 * 且 addRoute 不会重新解析已完成的导航（vue-router 只递增 routesVersion），
 * 于是冷启动直达 /design 永远停在登录墙。
 * → entry.ts 必须 await 本 promise 之后再装 router。
 */
export const designRoutesReady: Promise<void> = import.meta.env.DEV
  ? import("./design")
      .then(({ designRoutes }) => {
        for (const route of designRoutes) {
          router.addRoute(route);
        }
      })
      .catch((error: unknown) => {
        // 设计目录是开发期附属品：加载失败不得阻断应用启动
        console.error(
          "[router] design catalog route registration failed",
          error
        );
      })
  : Promise.resolve();

/**
 * 认证守卫：无任何凭据时把业务页重定向到登录页。
 *
 * 只判断凭据是否存在，不判断 access token 是否过期 —— 过期由 HTTP 客户端的
 * 单飞刷新处理；两者都缺失才视为未登录，避免刷新可用时被误踢。
 */
export const PUBLIC_LAYOUTS = new Set(["auth", "bare", "design"]);

/**
 * 设计目录路径前缀（与 src/router/design.ts 的路由表同源）。
 *
 * 设计系统目录对**匿名访客始终开放**：路由自带 meta.layout = "design"，
 * 正常路径下由 PUBLIC_LAYOUTS 放行。这里再按前缀兜一层，是让这条约定不依赖
 * 路由注册时序——即便设计目录模块加载失败、路由表尚未就位，匿名访客也只落
 * 404 占位页，而不会被挡在登录墙后面。
 */
const DESIGN_PATH_PREFIX = "/design";

function isDesignPath(path: string): boolean {
  return (
    path === DESIGN_PATH_PREFIX || path.startsWith(`${DESIGN_PATH_PREFIX}/`)
  );
}

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

  // 设计目录公开：设计与令牌目录任何人可看，不要求登录
  if (isDesignPath(to.path)) {
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
