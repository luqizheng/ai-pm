import {
  BookOpen,
  Folders,
  GitBranch,
  Home,
  Inbox,
  LayoutDashboard,
  ListChecks,
  Megaphone,
  ScanSearch,
  Settings2,
  Sparkles,
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
 *   4. admin —— 管理（默认收起）
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
}

/** 导航分组 */
export interface NavGroup {
  /** 分组标识 */
  id: string;
  /** i18n key（shell.app.side.group.*） */
  labelKey: string;
  /** 该组的导航项 */
  items: NavItem[];
  /** 默认收起（Figma 的 ADMINISTRATION 组） */
  defaultCollapsed?: boolean;
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
        icon: LayoutDashboard
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
      {
        path: "/project/repository",
        labelKey: "shell.app.side.item.repository",
        icon: GitBranch,
        expandable: true,
        children: [
          {
            path: "/project/repository/overview",
            labelKey: "shell.app.side.sub.overview"
          },
          {
            path: "/project/repository/branches",
            labelKey: "shell.app.side.sub.branches"
          }
        ]
      },
      {
        path: "/project/discovery",
        labelKey: "shell.app.side.item.discovery",
        icon: ScanSearch,
        expandable: true
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
      {
        path: "/project/execution",
        labelKey: "shell.app.side.item.execution",
        icon: Sparkles,
        badge: 2
      },
      {
        path: "/project/knowledge",
        labelKey: "shell.app.side.item.knowledge",
        icon: BookOpen
      },
      {
        path: "/project/devenv",
        labelKey: "shell.app.side.item.devenv",
        icon: Terminal
      }
    ]
  },
  {
    id: "admin",
    labelKey: "shell.app.side.group.admin",
    badge: 2,
    defaultCollapsed: true,
    items: [
      {
        path: "/admin/settings",
        labelKey: "shell.app.side.item.adminSettings",
        icon: Settings2
      },
      {
        path: "/admin/members",
        labelKey: "shell.app.side.item.adminMembers",
        icon: UsersRound
      },
      {
        path: "/admin/announcements",
        labelKey: "shell.app.side.item.adminAnnouncements",
        icon: Megaphone,
        component: () =>
          import("@/views/admin/AdminAnnouncementsView.vue").then(
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
 * 父路径命中其全部子路径（/project/repository 命中 /project/repository/branches），
 * 但**不得**让 /project/repository 命中 /project/repository-xxx 这类同前缀兄弟路径。
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
