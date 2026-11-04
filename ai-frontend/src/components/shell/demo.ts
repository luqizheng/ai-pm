/**
 * 应用壳演示数据（骨架阶段）。
 *
 * 本文件只提供**壳渲染所需的静态上下文**（当前组织 / 当前项目 / 仓库元数据），
 * 不含任何业务逻辑与后端接线；接入真实数据源时整体替换。
 *
 * 文案一律走 i18n（组件内 t(...)），此处只放专有名词（项目名 / 团队名等）。
 */
import type { ProjectSwitcherProject } from "@/components/global/project-switcher";

/** 当前组织（顶栏租户上下文指示；Shell V1.1 不做 Workspace Switcher） */
export const demoOrganization = {
  id: "org-acme-research",
  name: "Acme Research"
};

/**
 * 可选项目清单（项目切换器的条目）。
 *
 * Figma「04 — Global Navigation」Project Context Header / State=Open 的示例为
 * 「Atlas Platform」与「Research Sandbox」两个项目。
 */
export const demoProjects = [
  {
    id: "project-atlas-platform",
    name: "Atlas Platform",
    description: "Shared code and documentation for the platform team."
  },
  {
    id: "project-research-sandbox",
    name: "Research Sandbox",
    description: "Exploratory work for the research group."
  }
];

/** 当前项目（顶栏项目上下文 + 左导航项目头 + 右上下文栏） */
export const demoProject = {
  id: "project-atlas-platform",
  name: "Atlas Platform",
  organization: demoOrganization.name,
  description: "Shared code and documentation for the platform team."
};

/** 新版切换器候选（Figma 33:82961 global/project-switcher 规格；当前项即 demoProject）。
 *  与上方 demoProjects（旧壳 shell/ProjectSwitcher 消费）并存，顶栏切换新版后旧者待清理。 */
export const demoSwitcherProjects: ProjectSwitcherProject[] = [
  {
    id: demoProject.id,
    name: demoProject.name,
    group: "favorites"
  },
  {
    id: "project-sdk-toolkit",
    name: "SDK Toolkit",
    group: "recent",
    meta: "2h ago"
  },
  {
    id: "project-developer-portal",
    name: "Developer Portal",
    group: "recent",
    meta: "Yesterday"
  }
];

/** 当前仓库元数据（右上下文栏） */
export const demoRepository = {
  name: "acme / atlas-platform",
  visibility: "Private",
  defaultBranch: "main",
  role: "Contributor",
  owner: "Platform team",
  syncedAt: "Synced 2 min ago"
};

/** 右上下文栏「相关信息」链接 */
export const demoRelatedLinks = [
  { id: "guide", label: "Project guide" },
  { id: "contribution", label: "Contribution guidelines" }
];

/** 当前用户（顶栏账户菜单） */
export const demoUser = {
  id: "user-am",
  name: "Alex Morgan",
  initials: "AM"
};

/** AI 指令栏的作用域串（Figma：Atlas Platform / Repository / main） */
export const demoCommandScope = `${demoProject.name} / Repository / ${demoRepository.defaultBranch}`;

/** AI 指令栏建议指令 */
export const demoSuggestions = ["explain", "setup"] as const;

/**
 * 模拟「解析新项目上下文」过程。
 *
 * 真实实现里这里是拉取项目详情、成员权限与可用仓库；
 * 骨架阶段用一段短延时把 Figma「07」的切换中状态跑出来，
 * 接入真实数据源时整体替换为实际请求。
 */
export function resolveProjectSwitch(): Promise<void> {
  return new Promise(resolve => {
    setTimeout(resolve, 600);
  });
}
