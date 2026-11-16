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
  ListTree,
  Megaphone,
  Network,
  ScanSearch,
  Settings2,
  Sparkles,
  SquareKanban,
  Terminal,
  Users
} from "lucide-vue-next";
import type { Component } from "vue";

/**
 * 应用壳的导航配置（**菜单与路由的单源**）。
 *
 * R2-D1（V1.2）：单一左侧导航栏承载三种导航上下文，配置按上下文拆为三源，
 * 但**不是三套 Sidebar**——三源最终渲染进同一个 SideNav（区域顺序见
 * NAV_REGION_ORDER，存储顺序不决定视觉顺序）：
 *   1. globalNavConfig —— Personal / Team / 遗留管理入口
 *   2. projectNavConfig —— Current Project 区域（总控台置顶 + 业务模块居中）
 *   3. settingsNavConfig —— 项目设置（置底组；**仅供侧栏渲染**，其路由由
 *      router/index.ts 手工注册，buildRoutes 不消费本源）
 *
 * 路由表（src/router/index.ts）由 global + project 两源派生，避免
 * 「菜单与路由漂移」。navGroups 为兼容视图（既有契约测试与命令面板消费）。
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
   * 缺省时路由落到占位页（PLANNED 占位入口，如 Analysis / Product
   * Cognition——页面 Phase 4 落地后替换，占位能力不得伪装已上线）。
   * 用懒加载而非静态 import：视图会 import @/components/shell 聚合导出，
   * 而壳的 SideNav 又依赖本文件——静态 import 会形成循环依赖。
   */
  component?: () => Promise<Component>;
  /** 二级子菜单（Figma 的 Repository 上下文子菜单） */
  children?: NavChild[];
  /** 是否需要展开才显示子菜单（无 chevron 的纯链接项为 false） */
  expandable?: boolean;
  /** 规划项：目标页面未落地（渲染层须给出可见的规划标识） */
  planned?: boolean;
  /**
   * 遗留管理入口标记（NAV-0103 待核验归属；C1 起为数据标识，渲染层暂不消费）。
   * 带标记的项在归属裁决前维持现「管理」组原样呈现，不得包装为平台管理分区。
   */
  legacyPending?: boolean;
}

/** 二级子菜单项 */
export interface NavChild {
  /** 路由路径 */
  path: string;
  /** i18n key（shell.app.side.sub.*） */
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
  /** 该组是否属于「项目上下文」分组（下方带项目标记行） */
  projectScoped?: boolean;
  /** 无分组标题行（区域顶部的单条目组，如项目总控台） */
  headerless?: boolean;
}

/**
 * 导航区域排序规则（R2-D1 / V1.2 §6.1）：
 * Personal → Team → Current Project（条件显示）→ 遗留管理入口（尾部）。
 *
 * **存储顺序不决定视觉顺序**：三个配置源的数组顺序仅是存储约定；区域渲染
 * 顺序以本规则为准（SideNav 按此接线）。legacy 区两项带
 * NavItem.legacyPending 标记（NAV-0103 待核验），归属裁决前维持现
 * 「管理」组原样呈现，不得包装为平台管理分区。
 */
export const NAV_REGION_ORDER = [
  "personal",
  "team",
  "project",
  "adminLegacy"
] as const;

export type NavRegionId = (typeof NAV_REGION_ORDER)[number];

/** Personal 区域组 */
const personalGroup: NavGroup = {
  id: "personal",
  labelKey: "shell.app.side.group.personal",
  items: [
    {
      path: "/personal",
      labelKey: "shell.app.side.item.personalHome",
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
    }
  ]
};

/** Team 区域组（现「工作区」组） */
const workspaceGroup: NavGroup = {
  id: "workspace",
  labelKey: "shell.app.side.group.workspace",
  items: [
    {
      path: "/dashboard",
      labelKey: "shell.app.side.item.dashboard",
      icon: LayoutDashboard,
      // Team Dashboard（0044）：组织入口——团队档案/成员概览/项目入口，
      // 项目执行指标仍在项目总控台（UI 细则 #1）。
      component: () =>
        import("@/views/dashboard/TeamDashboardView.vue").then(
          module => module.default
        )
    },
    {
      path: "/projects",
      labelKey: "shell.app.side.item.projects",
      icon: Folders,
      component: () =>
        import("@/views/project/list/ProjectListView.vue").then(
          module => module.default
        )
    },
    // 团队设置（V1.2 Team 子树）：父项 redirect 首子项「团队常规」，
    // 旧落地页随子树化退役。Git 凭证为 PLANNED 占位（R-1：Team-owned
    // Credential，管理 UI 后续设计）；团队公告待 NAV-0103 核验，不进子树。
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
    }
  ]
};

/**
 * Current Project 区域：项目总控台（置顶单条，headerless 渲染）。
 * 原手工路由迁入导航树（NAV-02）：入口不再依赖注释中的「切换后落地页」。
 */
const projectHomeGroup: NavGroup = {
  id: "projectHome",
  labelKey: "shell.app.side.item.commandCenter",
  headerless: true,
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

/**
 * Current Project 区域：项目业务模块组（V1.2 §5 顺序居中）。
 *
 * 四域独立入口（P-03/P-09）：Analysis / Product Cognition 为 PLANNED 占位
 * 路由（页面 Phase 4 落地后替换）；KB 即 /project/knowledge 的最终定位
 * （迁移期页面仍三合一，不伪装已拆分）；Project Cognition 直连真页，
 * 原孤立路由（ORP-1）就此消除。开发环境已迁项目设置 → 开发环境（见
 * settingsNavConfig），旧路径 /project/devenv 重定向。
 */
const modulesGroup: NavGroup = {
  id: "modules",
  labelKey: "shell.app.side.group.modules",
  projectScoped: true,
  items: [
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
    // 执行中心（S05）：七页接真后端
    {
      path: "/project/execution",
      labelKey: "shell.app.side.item.execution",
      icon: Sparkles,
      component: () =>
        import("@/views/project/execution/ExecutionCenterView.vue").then(
          module => module.default
        )
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
    // Analysis（四域独立入口）：Phase 2 P2-01 起为真实页面——报告列表与详情
    // 独立承载于 views/project/analysis；报告页签随后续卡迁出 Knowledge（P2-06）。
    {
      path: "/project/analysis",
      labelKey: "shell.app.side.item.analysis",
      icon: Activity,
      component: () =>
        import("@/views/project/analysis/AnalysisHomeView.vue").then(
          module => module.default
        )
    },
    // KB（四域独立入口）：/project/knowledge 为 KB 最终定位（C-1），
    // 迁移期该页仍承载 KB+PC+报告三合一内容，不得伪装已拆分
    {
      path: "/project/knowledge",
      labelKey: "shell.app.side.item.knowledge",
      icon: BookOpen,
      component: () =>
        import("@/views/project/knowledge/KnowledgeView.vue").then(
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
    },
    // 人工审核中心（R01–R12）：统一人工决策收件箱，徽标经 reviewCenter
    // store 动态注入（SideNav itemBadge）。
    {
      path: "/project/review-center",
      labelKey: "review.nav.title",
      icon: ClipboardCheck,
      component: () =>
        import("@/views/project/review-center/ReviewCenterPage.vue").then(
          module => module.default
        )
    }
  ]
};

/** 遗留管理入口组（NAV-0103 待核验；C4 后仅剩两项，成员/AI Provider 已迁团队设置） */
const adminGroup: NavGroup = {
  id: "admin",
  labelKey: "shell.app.side.group.admin",
  badge: 2,
  items: [
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
 * 全局导航配置源（Personal / Team / 遗留管理入口）。
 *
 * C2 区域渲染与 C4 Team 迁移的接线面：消费方按 NAV_REGION_ORDER 取区域，
 * 不依赖本数组顺序。
 */
export const globalNavConfig: NavGroup[] = [
  personalGroup,
  workspaceGroup,
  adminGroup
];

/**
 * 项目导航配置源（Current Project 区域：总控台置顶 + 业务模块居中）。
 * buildRoutes 消费本源派生路由；项目设置组不在此（见 settingsNavConfig）。
 */
export const projectNavConfig: NavGroup[] = [projectHomeGroup, modulesGroup];

/**
 * 项目设置导航配置源（V1.2 §5 Current Project → 项目设置，置底组）。
 *
 * **仅供侧栏渲染接线，buildRoutes 不消费本源**——项目设置路由由
 * router/index.ts 手工注册（避免重复注册）。C3 同步手工注册
 * /project/settings/devenv（redirect → Devices）与 /project/settings/devenv
 * /executors（PLANNED 占位）；旧路径 /project/devenv 重定向到 Devices 新路径。
 * R-1 口径：不含独立 ssh-keys 条目（现有页面保留至后续迁入仓库分区）；
 * Repository 凭证绑定入口在仓库页面内预留（导航不设子项：NavChild 机制会把
 * 父项变为 redirect 首子，与真实父页面冲突）。
 */
export const settingsNavConfig: NavGroup = {
  id: "projectSettings",
  labelKey: "settings.project.scopeLabel",
  projectScoped: true,
  items: [
    {
      path: "/project/settings/general",
      labelKey: "settings.nav.projectGeneral",
      icon: Settings2,
      planned: true
    },
    {
      path: "/project/settings/members",
      labelKey: "settings.nav.projectMembers",
      icon: Users,
      planned: true
    },
    {
      path: "/project/settings/repositories",
      labelKey: "settings.nav.repositories",
      icon: GitBranch
    },
    {
      path: "/project/settings/devenv",
      labelKey: "shell.app.side.item.devenv",
      icon: Terminal,
      expandable: true,
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
    },
    {
      path: "/project/settings/ai",
      labelKey: "settings.nav.projectAi",
      icon: Sparkles,
      planned: true
    }
  ]
};

/**
 * 兼容视图：契约测试（navigation.test / router.test）与命令面板
 * （flatNavItems）消费的四组树。组合顺序对应 NAV_REGION_ORDER：
 * personal→Personal、workspace→Team、projectHome+modules→Current Project、
 * admin→遗留管理。settingsNavConfig 为渲染专用配置，不进本视图。
 */
export const navGroups: NavGroup[] = [
  personalGroup,
  workspaceGroup,
  ...projectNavConfig,
  adminGroup
];

/** 扁平化后的全部导航项（命令面板用） */
export const flatNavItems: NavItem[] = navGroups.flatMap(group => group.items);

/**
 * 判定某路径是否为导航项的激活态。
 *
 * 父路径命中其全部子路径（如 /project/discovery 命中其子路径），
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
 * 助手禁用路径规则（页面清单驱动；C4 4a 行为等价迁移）。
 *
 * 每条规则 = 精确命中或 `path + "/"` 前缀（覆盖子路由）。行为等价实证：
 * 迁移前快照与迁移后参数化断言双跑（见 C4 commit message）。
 * **禁止使用裸 `/settings/team` 前缀**——团队常规/落地页/凭证占位页对助手
 * 可见（4b 整域禁用待产品确认，确认前不得收紧）。
 */
const ASSISTANT_HIDDEN_RULES: Array<{ path: string }> = [
  // 平台设置：租户运营面（现状禁用）
  { path: "/admin/settings" },
  // 公告管理：现状禁用；迁团队后仍禁用
  { path: "/admin/announcements" },
  // 项目设置：配置面，完整前缀（含全部子路由与 PLANNED 占位）
  { path: "/project/settings" },
  // 团队成员：原 /admin/members 迁入，行为等价禁用
  { path: "/settings/team/members" },
  // 团队 AI Provider：原 admin 组项迁出导航 admin 组，行为等价禁用
  { path: "/settings/team/ai-providers" }
];

/**
 * 判定某路径是否为「管理」面（助手禁用判定，页面清单驱动）。
 *
 * C4 前实现从 admin 导航组动态派生；C4 起 admin 组仅剩两项遗留入口且
 * Team 页面已迁出 admin 组，改为显式规则清单。语义不变：这是 UX 可见性
 * 判定，**不是权限控制**（菜单隐藏不替代后端授权）。
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
