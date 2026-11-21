import {
  Activity,
  BookOpen,
  Brain,
  ClipboardCheck,
  Folders,
  Gauge,
  GitBranch,
  Bell,
  Home,
  LayoutDashboard,
  ListChecks,
  Package,
  ListTree,
  Megaphone,
  Network,
  ScanSearch,
  Settings2,
  Sparkles,
  SquareKanban,
  Terminal
} from "lucide-vue-next";
import type { Component } from "vue";

/**
 * 应用壳的导航配置（**菜单与路由的单源**）。
 *
 * 三层导航模型（Figma「三层」画布 157:3603 · Global Navigation — Level 3）：
 *   L1 顶栏上下文 —— Team/Project 切换器是上下文选择器而非导航层级
 *      （"CONTEXT SELECTOR · NOT A NAVIGATION LEVEL"）；实现在 TopBar 的
 *      TeamSwitcher + ProjectSwitcher，侧栏不内嵌切换器。
 *   L2 侧栏业务域分组 —— 本文件唯一渲染源 mainNavGroups（扁平组列表）：
 *      全局 → 项目总览 → 开发 → 知识 → 环境 → 管理；四个项目域组
 *      （projectScoped）仅在选中项目时渲染（shell.currentProjectId 单源，
 *      R2-D4 沿用）；管理组默认折叠（Figma ADMINISTRATION Collapsed）。
 *   L3 页内页签 —— 各业务页自承载（RouterLink 页签条 / ?tab= Tabs），
 *      不进侧栏。
 *
 * 路由表（src/router/index.ts）由 mainNavGroups + auxiliaryNavItems 派生：
 *   - auxiliaryNavItems：Figma 未画的既有页面（团队总览/技术设计/任务管理/
 *     分析/两认知域）——撤出主菜单但路由保留，命令面板可直达；
 *   - manualRoute 条目（仓库/开发环境，路径在 /project/settings 前缀下，
 *     随项目设置域手工注册）：buildRoutes 跳过，避免重复注册；助手可见性
 *     前缀判定（/project/settings）不随菜单归属变化（AGENTS 规则 5）。
 */

/** 导航项：一个可点击目的地（可带子菜单） */
export interface NavItem {
  /** 路由路径（同时作为菜单项的稳定标识） */
  path: string;
  /** i18n key（shell.app.side.item.*） */
  labelKey: string;
  /** lucide 图标组件 */
  icon: Component;
  /** 右上角徽标数字（如收件箱未读数） */
  badge?: number;
  /**
   * 业务页组件（懒加载）。
   *
   * 缺省时路由落到占位页（PLANNED 占位入口——页面落地后替换，占位能力
   * 不得伪装已上线）。用懒加载而非静态 import：视图会 import
   * @/components/shell 聚合导出，而壳的 SideNav 又依赖本文件——静态
   * import 会形成循环依赖。
   */
  component?: () => Promise<Component>;
  /** 二级子菜单（如开发环境 Devices/Executors/Runners） */
  children?: NavChild[];
  /** 是否需要展开才显示子菜单（无 chevron 的纯链接项为 false） */
  expandable?: boolean;
  /** 规划项：目标页面未落地（渲染层须给出可见的规划标识） */
  planned?: boolean;
  /**
   * 遗留管理入口标记（NAV-0103 待核验归属）。带标记的项在归属裁决前
   * 维持原样呈现，不得包装为平台管理分区。
   */
  legacyPending?: boolean;
  /**
   * 路由由 router/index.ts 手工注册（路径位于 /project/settings 前缀，
   * 随项目设置域一起注册）；buildRoutes 跳过本条目避免重复注册。
   */
  manualRoute?: boolean;
}

/** 二级子菜单项 */
export interface NavChild {
  /** 路由路径 */
  path: string;
  /** i18n key（shell.app.side.item.*） */
  labelKey: string;
  /** 业务页组件（懒加载）；缺省 = 占位页 */
  component?: () => Promise<Component>;
  /** 规划项（渲染层须给出可见的规划标识） */
  planned?: boolean;
}

/** 导航分组 */
export interface NavGroup {
  /** 分组标识 */
  id: string;
  /** i18n key（shell.app.side.group.*） */
  labelKey: string;
  /** 该组的导航项 */
  items: NavItem[];
  /** 组标题右侧徽标 */
  badge?: number;
  /** 该组是否属于项目域分组（仅在选中项目时渲染，R2-D4） */
  projectScoped?: boolean;
  /** 分组默认折叠（Figma ADMINISTRATION Collapsed 态） */
  defaultCollapsed?: boolean;
}

/* ─── L2 主菜单组（Figma 三层 IA，顺序即视觉顺序） ─── */

/** 全局组：跨项目入口（末条「全局搜索」为动作项，由 SideNav 渲染层追加） */
const globalGroup: NavGroup = {
  id: "global",
  labelKey: "shell.app.side.group.global",
  items: [
    {
      path: "/personal",
      labelKey: "shell.app.side.item.personalWorkspace",
      icon: Home,
      component: () =>
        import("@/views/personal/PersonalHomeView.vue").then(
          module => module.default
        )
    },
    {
      path: "/notifications",
      labelKey: "shell.app.side.item.notifications",
      icon: Bell
    },
    {
      path: "/projects",
      labelKey: "shell.app.side.item.projects",
      icon: Folders,
      component: () =>
        import("@/views/project/list/ProjectListView.vue").then(
          module => module.default
        )
    }
  ]
};

/** 项目总览组：单条目组（原总控台置顶单条改为 Figma 分组呈现） */
const projectOverviewGroup: NavGroup = {
  id: "projectOverview",
  labelKey: "shell.app.side.group.projectOverview",
  projectScoped: true,
  items: [
    {
      path: "/project/command-center",
      labelKey: "shell.app.side.item.commandCenter",
      icon: Gauge,
      component: () =>
        import("@/views/project/command-center/CommandCenterView.vue").then(
          module => module.default
        )
    }
  ]
};

/** 开发组：仓库（手工路由）在前，业务模块随 Figma 顺序 */
const developmentGroup: NavGroup = {
  id: "development",
  labelKey: "shell.app.side.group.development",
  projectScoped: true,
  items: [
    // 仓库（Figma DEVELOPMENT 首项）：路由随项目设置域手工注册；
    // R-1 口径——Repository 凭证绑定入口在仓库页面内预留
    {
      path: "/project/settings/repositories",
      labelKey: "shell.app.side.item.repository",
      icon: GitBranch,
      manualRoute: true
    },
    // 代码发现（S01-I2）：AI 只产 finding，人工确认后入库
    {
      path: "/project/discovery",
      labelKey: "shell.app.side.item.discovery",
      icon: ScanSearch,
      component: () =>
        import("@/views/project/discovery/DiscoveryView.vue").then(
          module => module.default
        )
    },
    // 需求（评审/基线/历史/影响为下钻页，路由见 index.ts 手工段）
    {
      path: "/project/requirements",
      labelKey: "shell.app.side.item.requirements",
      icon: ListChecks,
      component: () =>
        import("@/views/project/requirements/RequirementsView.vue").then(
          module => module.default
        )
    },
    // AI 执行（S05）：七页接真后端
    {
      path: "/project/execution",
      labelKey: "shell.app.side.item.execution",
      icon: Sparkles,
      component: () =>
        import("@/views/project/execution/ExecutionCenterView.vue").then(
          module => module.default
        )
    }
  ]
};

/** 知识组：KB + 人工审核（知识评审，徽标经 reviewCenter store 动态注入） */
const knowledgeGroup: NavGroup = {
  id: "knowledge",
  labelKey: "shell.app.side.group.knowledge",
  projectScoped: true,
  items: [
    // KB：/project/knowledge 为 KB 最终定位（C-1），迁移期该页仍承载
    // KB+PC+报告三合一内容（页内 ?tab= 切换），不得伪装已拆分
    {
      path: "/project/knowledge",
      labelKey: "shell.app.side.item.knowledge",
      icon: BookOpen,
      component: () =>
        import("@/views/project/knowledge/KnowledgeView.vue").then(
          module => module.default
        )
    },
    // 知识评审（Figma Knowledge Review）= 人工审核中心（R01–R12）：
    // 统一人工决策收件箱，动态徽标随当前项目聚合（SideNav itemBadge）
    {
      path: "/project/review-center",
      labelKey: "shell.app.side.item.knowledgeReview",
      icon: ClipboardCheck,
      component: () =>
        import("@/views/project/review-center/ReviewCenterPage.vue").then(
          module => module.default
        )
    }
  ]
};

/** 环境组：开发环境（Devices/Executors/Runners 子菜单保留，手工路由） */
const environmentGroup: NavGroup = {
  id: "environment",
  labelKey: "shell.app.side.group.environment",
  projectScoped: true,
  items: [
    {
      path: "/project/settings/devenv",
      labelKey: "shell.app.side.item.devenv",
      icon: Terminal,
      expandable: true,
      manualRoute: true,
      children: [
        {
          path: "/project/settings/devenv/devices",
          labelKey: "shell.app.side.item.devenvDevices"
        },
        {
          path: "/project/settings/devenv/executors",
          labelKey: "shell.app.side.item.devenvExecutors",
          planned: true
        },
        {
          path: "/project/settings/runners",
          labelKey: "shell.app.side.item.devenvRunners"
        }
      ]
    }
  ]
};

/** 管理组：团队设置子树 + 遗留管理入口；默认折叠（Figma Collapsed） */
const administrationGroup: NavGroup = {
  id: "administration",
  labelKey: "shell.app.side.group.administration",
  defaultCollapsed: true,
  items: [
    // 团队设置（V1.2 Team 子树）：父项 redirect 首子项「团队常规」。
    // Git 凭证为 PLANNED 占位（R-1：Team-owned Credential，管理 UI 后续
    // 设计）；团队公告待 NAV-0103 核验，不进子树。
    {
      path: "/settings/team",
      labelKey: "shell.app.side.item.teamSettings",
      icon: Settings2,
      expandable: true,
      children: [
        {
          path: "/settings/team/general",
          labelKey: "settings.nav.general",
          component: () =>
            import("@/views/settings/team/SettingsTeamGeneralView.vue").then(
              module => module.default
            )
        },
        {
          path: "/settings/team/members",
          labelKey: "settings.nav.teamMembers",
          component: () =>
            import("@/views/settings/team/SettingsTeamMembersView.vue").then(
              module => module.default
            )
        },
        {
          path: "/settings/team/products",
          labelKey: "settings.nav.products",
          component: () =>
            import("@/views/settings/team/products/ProductsListView.vue").then(
              module => module.default
            )
        },
        {
          path: "/settings/team/ai-providers",
          labelKey: "shell.app.side.item.aiProviders",
          component: () =>
            import("@/views/settings/SettingsTeamAiView.vue").then(
              module => module.default
            )
        },
        {
          path: "/settings/team/credentials",
          labelKey: "shell.app.side.item.gitCredentials",
          planned: true
        }
      ]
    },
    {
      path: "/admin/settings",
      labelKey: "shell.app.side.item.adminSettings",
      icon: Settings2,
      // 遗留管理入口：NAV-0103 待核验归属，裁决前原样保留（不迁移）
      legacyPending: true,
      component: () =>
        import("@/views/admin/AdminSettingsView.vue").then(
          module => module.default
        )
    },
    {
      path: "/admin/announcements",
      labelKey: "shell.app.side.item.adminAnnouncements",
      icon: Megaphone,
      // 遗留管理入口：NAV-0103 待核验归属（含迁团队后仍禁用助手的语义），裁决前原样保留
      legacyPending: true,
      component: () =>
        import("@/views/admin/AdminAnnouncementsView.vue").then(
          module => module.default
        )
    }
  ]
};

/**
 * L2 主菜单渲染源（Figma 三层 IA 顺序；SideNav 扁平渲染）。
 */
export const mainNavGroups: NavGroup[] = [
  globalGroup,
  projectOverviewGroup,
  developmentGroup,
  knowledgeGroup,
  environmentGroup,
  administrationGroup
];

/**
 * 撤出主菜单但**路由保留**的既有页面（严格按 Figma 条目口径）。
 * 不渲染进侧栏；buildRoutes 照常派生路由，命令面板（WorkspaceShell
 * searchCommands）据此提供直达入口。
 */
export const auxiliaryNavItems: NavItem[] = [
  // 团队总览（0044）：登录落地 redirect 目标（Q-08：只改名不改路径）
  {
    path: "/dashboard",
    labelKey: "shell.app.side.item.dashboard",
    icon: LayoutDashboard,
    component: () =>
      import("@/views/dashboard/TeamDashboardView.vue").then(
        module => module.default
      )
  },
  // 技术设计（S03）：分析 / 技术设计 / 架构决策 / 实施准备四个子页
  {
    path: "/project/design",
    labelKey: "shell.app.side.item.technicalDesign",
    icon: ListTree,
    component: () =>
      import("@/views/project/design/analysis/DesignAnalysisView.vue").then(
        module => module.default
      )
  },
  // 任务管理（S04）：任务拆解 / 任务池 / 执行规划 / 执行准备的工作区
  {
    path: "/project/tasks",
    labelKey: "shell.app.side.item.taskManagement",
    icon: SquareKanban,
    component: () =>
      import("@/views/project/tasks/overview/TasksOverviewView.vue").then(
        module => module.default
      )
  },
  // PA-07 执行看板（Figma 179:3603 00 Dashboard）：四块执行面板
  {
    path: "/project/dashboard",
    labelKey: "shell.app.side.item.executionDashboard",
    icon: Gauge,
    component: () =>
      import("@/views/project/dashboard/ExecutionDashboardView.vue").then(
        module => module.default
      )
  },
  // PA-07 产品面（Figma 179:3603 E 系列）：项目关联产品 + 生命周期门禁
  {
    path: "/project/products",
    labelKey: "shell.app.side.item.products",
    icon: Package,
    component: () =>
      import("@/views/project/products/ProductsView.vue").then(
        module => module.default
      )
  },
  // Analysis（四域独立入口）：报告列表与详情独立承载（P2-01）
  {
    path: "/project/analysis",
    labelKey: "shell.app.side.item.analysis",
    icon: Activity,
    component: () =>
      import("@/views/project/analysis/AnalysisHomeView.vue").then(
        module => module.default
      )
  },
  // Product Cognition（四域独立入口）：页面 Phase 4 落地，PLANNED 占位路由
  {
    path: "/project/product-cognition",
    labelKey: "shell.app.side.item.productCognition",
    icon: Brain,
    planned: true
  },
  // Project Cognition（四域独立入口）：真页直连（人工启动 Run + 逐项确认）
  {
    path: "/project/cognition",
    labelKey: "shell.app.side.item.projectCognition",
    icon: Network,
    component: () =>
      import("@/views/project/cognition/CognitionPage.vue").then(
        module => module.default
      )
  }
];

/** 扁平化后的全部导航项（命令面板与设计套件预览用） */
export const flatNavItems: NavItem[] = [
  ...mainNavGroups.flatMap(group => group.items),
  ...auxiliaryNavItems
];

/**
 * 判定某路径是否为导航项的激活态。
 *
 * 父路径命中其全部子路径（如 /project/settings/devenv 命中其子路径），
 * 但**不得**让父路径命中 /project/discovery-xxx 这类同前缀兄弟路径。
 */
export function isNavItemActive(item: NavItem, currentPath: string): boolean {
  if (currentPath === item.path) return true;
  if (!currentPath.startsWith(`${item.path}/`)) return false;
  return item.children?.length
    ? item.children.some(child => currentPath === child.path)
    : true;
}

/** 判定某子菜单项是否为激活态 */
export function isNavChildActive(
  child: NavChild,
  currentPath: string
): boolean {
  return currentPath === child.path;
}

/**
 * 助手禁用路径规则（页面清单驱动；行为不变）。
 *
 * 每条规则 = 精确命中或 `path + "/"` 前缀（覆盖子路由）。
 * **禁止使用裸 `/settings/team` 前缀**——团队常规/落地页/凭证占位页对助手
 * 可见（4b 整域禁用待产品确认，确认前不得收紧）。
 */
const ASSISTANT_HIDDEN_RULES: Array<{ path: string }> = [
  // 平台设置：租户运营面（现状禁用）
  { path: "/admin/settings" },
  // 公告管理：现状禁用；迁团队后仍禁用
  { path: "/admin/announcements" },
  // 项目设置：配置面，完整前缀（含全部子路由与 PLANNED 占位；
  // 主菜单「仓库」「开发环境」条目路径在本前缀下，助手语义随路径不变）
  { path: "/project/settings" },
  // 团队成员：原 /admin/members 迁入，行为等价禁用
  { path: "/settings/team/members" },
  // 团队 AI Provider：原 admin 组项迁出导航 admin 组，行为等价禁用
  { path: "/settings/team/ai-providers" }
];

/**
 * 判定某路径是否为「管理」面（助手禁用判定，页面清单驱动）。
 *
 * 语义不变：这是 UX 可见性判定，**不是权限控制**（菜单隐藏不替代后端授权）。
 */
export function isManagementPath(currentPath: string): boolean {
  return ASSISTANT_HIDDEN_RULES.some(
    rule => currentPath === rule.path || currentPath.startsWith(`${rule.path}/`)
  );
}

/**
 * 判定某路径是否不出现 AI 助手：管理/配置面规则清单命中。
 */
export function isAssistantHiddenPath(currentPath: string): boolean {
  return isManagementPath(currentPath);
}
