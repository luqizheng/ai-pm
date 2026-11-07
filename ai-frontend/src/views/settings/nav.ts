import type { Component } from "vue";
import {
  Bell,
  Cpu,
  GitBranch,
  Mail,
  Monitor,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Users,
  UserRound
} from "lucide-vue-next";

import { PERSONAL_NAV, type SettingsNavEntry } from "@/components/settings";

/**
 * 应用设置域导航（个人域）。
 *
 * 单一事实源仍是 settings.model 的 PERSONAL_NAV（条目 id 与顺序），
 * 此处仅把设计目录路径前缀改写为应用路径；条目 id 同时是
 * i18n key（settings.nav.<id>）与侧栏图标键。
 */
export const APP_SETTINGS_ROOT = "/settings";

/** 个人域应用侧栏条目（/settings/personal/*；frame 为设计目录索引元数据，不进应用） */
export const PERSONAL_APP_NAV: SettingsNavEntry[] = PERSONAL_NAV.map(entry => ({
  id: entry.id,
  proposed: entry.proposed,
  to: entry.to.replace(/^\/design\/settings/, APP_SETTINGS_ROOT)
}));

/** 侧栏图标（与条目 id 对应；同设计目录落地页五图标） */
export const PERSONAL_ICONS: Record<string, Component> = {
  profile: UserRound,
  security: ShieldCheck,
  preferences: SlidersHorizontal,
  notifications: Bell,
  ai: Sparkles
};

/** 团队域应用侧栏条目（/settings/team/*）。设计目录 tenant 四类中仅
 *  AI 配置（Provider 管理，0041）先落地应用，其余条目随后端能力逐个迁入。 */
export const TEAM_APP_NAV: SettingsNavEntry[] = [
  { id: "members", to: "/settings/team/members" },
  { id: "invitations", to: "/settings/team/invitations" },
  { id: "general", to: "/settings/team/general" },
  { id: "aiProviders", to: "/settings/team/ai-providers" }
];

/** 团队域侧栏图标 */
export const TEAM_ICONS: Record<string, Component> = {
  members: Users,
  invitations: Mail,
  general: Settings2,
  aiProviders: Cpu
};

/** 项目域应用侧栏条目（/project/settings/*，V1.0 稿五节，2026-11-07）。
 *
 * Git Repositories / Runners 两节由原独立页面迁入（真数据）；General、
 * Members、AI Configuration 三节规划中（proposed 徽标 + 占位页），随后端
 * 能力逐个迁入。条目 id 同样是 i18n key（settings.nav.<id>）。 */
export const PROJECT_SETTINGS_ROOT = "/project/settings";

export const PROJECT_APP_NAV: SettingsNavEntry[] = [
  { id: "projectGeneral", to: "/project/settings/general", proposed: true },
  { id: "projectMembers", to: "/project/settings/members", proposed: true },
  { id: "repositories", to: "/project/settings/repositories" },
  { id: "runners", to: "/project/settings/runners" },
  { id: "projectAi", to: "/project/settings/ai", proposed: true }
];

/** 项目域侧栏图标 */
export const PROJECT_ICONS: Record<string, Component> = {
  projectGeneral: Settings2,
  projectMembers: Users,
  repositories: GitBranch,
  runners: Monitor,
  projectAi: Sparkles
};
