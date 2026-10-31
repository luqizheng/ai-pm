import type { Component } from "vue";
import {
  Activity,
  Bot,
  BookOpen,
  Cpu,
  FileText,
  ListTree,
  Map,
  PackageCheck,
  Scale,
  Users
} from "lucide-vue-next";

/**
 * 工作空间导航模式（用户裁决 2026-11-01）：
 *   - classic：传统模式，9 项（工作空间组 4 + 研发模块组 5）
 *   - ai：V1.2 UI，5 项（工作空间组单组，五个核心研发工作模式）
 *
 * 两种模式共用同一套 /workspace/:projectId/<slug> 路由；模式只决定左栏展示哪些
 * 入口，不改变已注册的页面。顶栏模式段控件在同一壳内切换（不跳 /dashboard）。
 */
export type NavMode = "classic" | "ai";

export interface NavItemDef {
  key: string;
  /** /workspace/:projectId/<slug> 的 slug */
  slug: string;
  labelKey: string;
  icon: Component;
}

export interface NavGroupDef {
  key: string;
  labelKey: string;
  items: NavItemDef[];
}

const classicGroups: NavGroupDef[] = [
  {
    key: "workspace",
    labelKey: "workspace.nav.groupWorkspace",
    items: [
      {
        key: "console",
        slug: "console",
        labelKey: "workspace.nav.console",
        icon: Map
      },
      {
        key: "team",
        slug: "team",
        labelKey: "workspace.nav.team",
        icon: Users
      },
      {
        key: "goals",
        slug: "goals",
        labelKey: "workspace.nav.goals",
        icon: ListTree
      },
      {
        key: "activity",
        slug: "activity",
        labelKey: "workspace.nav.activity",
        icon: Activity
      }
    ]
  },
  {
    key: "modules",
    labelKey: "workspace.nav.groupModules",
    items: [
      {
        key: "requirements",
        slug: "requirements",
        labelKey: "workspace.nav.requirements",
        icon: FileText
      },
      {
        key: "decisions",
        slug: "decisions",
        labelKey: "workspace.nav.decisions",
        icon: Scale
      },
      {
        key: "knowledge",
        slug: "knowledge",
        labelKey: "workspace.nav.knowledge",
        icon: BookOpen
      },
      {
        key: "artifacts",
        slug: "artifacts",
        labelKey: "workspace.nav.artifacts",
        icon: PackageCheck
      },
      {
        key: "models",
        slug: "models",
        labelKey: "workspace.nav.models",
        icon: Cpu
      }
    ]
  }
];

const aiGroups: NavGroupDef[] = [
  {
    key: "workspace",
    labelKey: "workspace.nav.groupWorkspace",
    items: [
      {
        key: "goals",
        slug: "goals",
        labelKey: "workspace.nav.goalsPlan",
        icon: ListTree
      },
      {
        key: "execution",
        slug: "execution",
        labelKey: "workspace.nav.execution",
        icon: Bot
      },
      {
        key: "decisions",
        slug: "decisions",
        labelKey: "workspace.nav.decisions",
        icon: Scale
      },
      {
        key: "knowledge",
        slug: "knowledge",
        labelKey: "workspace.nav.projectCognition",
        icon: BookOpen
      },
      {
        key: "artifacts",
        slug: "artifacts",
        labelKey: "workspace.nav.artifacts",
        icon: PackageCheck
      }
    ]
  }
];

export const NAV_GROUPS: Record<NavMode, NavGroupDef[]> = {
  classic: classicGroups,
  ai: aiGroups
};

/** 各模式的默认落点页（切模式时当前页在目标模式不存在，回退到此） */
export const NAV_DEFAULT_SLUG: Record<NavMode, string> = {
  classic: "console",
  ai: "goals"
};

export function navSlugs(mode: NavMode): string[] {
  return NAV_GROUPS[mode].flatMap(group => group.items.map(item => item.slug));
}

export function navDefaultSlug(mode: NavMode): string {
  return NAV_DEFAULT_SLUG[mode];
}
