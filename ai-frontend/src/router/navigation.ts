import {
  BookOpen,
  Cpu,
  ListTree,
  Folders,
  Home,
  Inbox,
  LayoutDashboard,
  ListChecks,
  Megaphone,
  ScanSearch,
  Settings2,
  Sparkles,
  SquareKanban,
  Terminal,
  UsersRound
} from "lucide-vue-next";
import type { Component } from "vue";

/**
 * 应用壳的导航树（**菜单与路由的单源**）。
 *
 * Figma「03 — Application Shell」左导航分四组，三种上下文不得混成一张扁平菜单
 * （Shell V1.1 §2B）：
 *   1. personal —— 个人工作区（跨项目）
 *   2. workspace —— 全局工作区（项目列表 / 总览）
 *   3. modules —— 当前项目下的业务模块（**仅在选中项目时出现**）
 *   4. admin —— 管理
 *
 * 路由表（src/router/index.ts）由本文件派生，避免「菜单与路由漂移」。
 * 演示期文案走 i18n key（shell.app.side.item.*），标题留空时取该 key。
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
   * 缺省时路由落到占位页；已落地的业务页在此登记，保持「导航树 = 菜单与路由的单源」。
   * 用懒加载而非静态 import：视图会 import @/components/shell 聚合导出，
   * 而壳的 SideNav 又依赖本文件 —— 静态 import 会形成循环依赖。
   */
  component?: () => Promise<Component>;
  /** 二级子菜单（Figma 的 Repository 上下文子菜单） */
  children?: NavChild[];
  /** 是否需要展开才显示子菜单（无 chevron 的纯链接项为 false） */
  expandable?: boolean;
}

/** 二级子菜单项 */
export interface NavChild {
  /** 路由路径 */
  path: string;
  /** i18n key（shell.app.side.sub.*） */
  labelKey: string;
  /** 业务页组件（懒加载）；缺省 = 占位页 */
  component?: () => Promise<Component>;
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
}

/** 导航分组顺序 = Figma 视觉顺序 */
export const navGroups: NavGroup[] = [
  {
    id: "personal",
    labelKey: "shell.app.side.group.personal",
    items: [
      {
        path: "/personal",
        labelKey: "shell.app.side.item.personalHome",
        icon: Home,
        component: () => import("@/views/personal/PersonalHomeView.vue")
      },
      {
        path: "/inbox",
        labelKey: "shell.app.side.item.inbox",
        icon: Inbox,
        badge: 3
      }
    ]
  },
  {
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
      }
    ]
  },
  {
    id: "modules",
    labelKey: "shell.app.side.group.modules",
    projectScoped: true,
    items: [
      // Git 仓库管理已迁入 Project Setting（/project/settings/repositories，
      // V1.0 稿「统一入口」裁决，2026-11-07）：旧路径 /project/repository/*
      // 在 router/index.ts 重定向。
      // 代码发现（S01-I2，2026-11-10）：接真 discovery sessions 系。
      {
        path: "/project/discovery",
        labelKey: "shell.app.side.item.discovery",
        icon: ScanSearch,
        component: () =>
          import("@/views/project/discovery/DiscoveryView.vue").then(
            module => module.default
          )
      },
      {
        path: "/project/requirements",
        labelKey: "shell.app.side.item.requirements",
        icon: ListChecks,
        component: () =>
          import("@/views/project/requirements/RequirementsView.vue").then(
            module => module.default
          )
      },
      // 技术设计（S03，docs/ui/s03，2026-11-10）：分析 / 技术设计 / 架构决策 /
      // 实施准备四个子页，全部接真后端（/v1/pm/designs · adrs · design-* 系）。
      {
        path: "/project/design",
        labelKey: "shell.app.side.item.technicalDesign",
        icon: ListTree,
        component: () =>
          import("@/views/project/design/analysis/DesignAnalysisView.vue").then(
            module => module.default
          )
      },
      // 任务管理（S04，docs/ui/s04，2026-11-10）：任务拆解 / 任务池 / 执行
      // 规划 / 执行准备的工作区。总览与任务池接真（/v1/pm/tasks 系），
      // 其余子页在对应接口落地前走 mock-preview 或占位页（router/index.ts）。
      {
        path: "/project/tasks",
        labelKey: "shell.app.side.item.taskManagement",
        icon: SquareKanban,
        component: () =>
          import("@/views/project/tasks/overview/TasksOverviewView.vue").then(
            module => module.default
          )
      },
      // 执行中心（S05，docs/ui/s05，2026-11-10）：七页接真后端。
      {
        path: "/project/execution",
        labelKey: "shell.app.side.item.execution",
        icon: Sparkles,
        component: () =>
          import("@/views/project/execution/ExecutionCenterView.vue").then(
            module => module.default
          )
      },
      // AI 会话入口已合并进 TopBar「助手」按钮（AI Chat 常驻列），
      // 主导航不再单列「AI 助手」；/project/assistant 路由保留供深链。
      {
        path: "/project/knowledge",
        labelKey: "shell.app.side.item.knowledge",
        icon: BookOpen,
        component: () =>
          import("@/views/project/knowledge/KnowledgeView.vue").then(
            module => module.default
          )
      },
      {
        path: "/project/devenv",
        labelKey: "shell.app.side.item.devenv",
        icon: Terminal,
        component: () =>
          import("@/views/project/devenv/BootstrapView.vue").then(
            module => module.default
          )
      }
      // 执行环境（Runners）已迁入 Project Setting（/project/settings/runners，
      // 与仓库同批统一入口）：旧路径 /project/environments* 重定向。
    ]
  },
  {
    id: "admin",
    labelKey: "shell.app.side.group.admin",
    badge: 2,
    items: [
      {
        path: "/admin/settings",
        labelKey: "shell.app.side.item.adminSettings",
        icon: Settings2,
        component: () =>
          import("@/views/admin/AdminSettingsView.vue").then(
            module => module.default
          )
      },
      {
        path: "/admin/members",
        labelKey: "shell.app.side.item.adminMembers",
        icon: UsersRound,
        component: () =>
          import("@/views/settings/team/SettingsTeamMembersView.vue").then(
            module => module.default
          )
      },
      {
        path: "/admin/announcements",
        labelKey: "shell.app.side.item.adminAnnouncements",
        icon: Megaphone,
        component: () =>
          import("@/views/admin/AdminAnnouncementsView.vue").then(
            module => module.default
          )
      },
      // 团队级 LLM Provider 管理（0041 Team LLM Provider）：路由随导航树派生
      // （原 router/index.ts 手工注册已迁入，保持「导航树 = 菜单与路由的单源」）。
      {
        path: "/settings/team/ai-providers",
        labelKey: "shell.app.side.item.aiProviders",
        icon: Cpu,
        component: () =>
          import("@/views/settings/SettingsTeamAiView.vue").then(
            module => module.default
          )
      }
    ]
  }
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
 * 判定某路径是否为「管理」分组下的页面（admin 组全部导航项 + 子路径）。
 *
 * 管理页（平台设置 / 成员 / 公告 / AI Provider）是租户级运营面，没有项目
 * 上下文，AI 助手（壳层 AI Chat 列、顶栏助手入口）在这些页面不出现。
 * 判定从导航树派生，保持「导航树 = 菜单与路由的单源」。
 */
export function isManagementPath(currentPath: string): boolean {
  const admin = navGroups.find(group => group.id === "admin");
  if (!admin) return false;
  return admin.items.some(item => {
    const roots = [item.path, ...(item.children?.map(c => c.path) ?? [])];
    return roots.some(
      path => currentPath === path || currentPath.startsWith(`${path}/`)
    );
  });
}
/**
 * 不出现 AI 助手的路径前缀（壳层 chrome 可见性的单源输入）。
 *
 * 项目设置（/project/settings/*）是配置面，与租户级管理页同类，不承载
 * 项目 AI 会话上下文：顶栏「助手」入口、AI Chat 列与右上下文栏均不出现。
 */
const ASSISTANT_HIDDEN_PREFIXES = ["/project/settings"];

/**
 * 判定某路径是否不出现 AI 助手：管理组页面 + 配置面路径前缀。
 */
export function isAssistantHiddenPath(currentPath: string): boolean {
  if (isManagementPath(currentPath)) return true;
  return ASSISTANT_HIDDEN_PREFIXES.some(
    path => currentPath === path || currentPath.startsWith(`${path}/`)
  );
}
