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

/** 当前项目（顶栏项目上下文 + 左导航项目标记 + 右上下文栏） */
export const demoProject = {
  id: "project-atlas-platform",
  name: "Atlas Platform",
  organization: demoOrganization.name,
  description: "Shared code and documentation for the platform team."
};

/** 项目切换器候选（Figma 33:82961 规格演示数据；当前项即 demoProject） */
export const demoProjects: ProjectSwitcherProject[] = [
  {
    id: demoProject.id,
    name: demoProject.name,
    group: "favorites"
  },
  { id: "project-sdk-toolkit", name: "SDK Toolkit", group: "recent", meta: "2h ago" },
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

/** 顶栏通知未读数 */
export const demoUnreadNotifications = 2;

/** AI 指令栏的作用域串（Figma：Atlas Platform / Repository / main） */
export const demoCommandScope = `${demoProject.name} / Repository / ${demoRepository.defaultBranch}`;

/** AI 指令栏建议指令 */
export const demoSuggestions = ["explain", "setup"] as const;
