import { computed, reactive } from "vue";

/**
 * Settings Framework 领域模型（设计目录演示用）。
 *
 * 规格来源：「AI R&D Workspace — Settings Framework V1.0」§4/§9：
 *   - 三个配置域（personal / tenant / project）彼此独立；
 *   - 一个设置项恰好属于一个主域（A setting must belong to exactly one
 *     primary scope）；
 *   - 域内条目按授权呈现三种访问态：可编辑 / 只读 / 不可见；
 *   - 产品能力未经确认的条目标记 Proposed，不得冒充已实现功能。
 *
 * 本文件同时承载设计目录的**演示角色 fixture**（谁能看/谁能改），
 * 仅服务于 `/design/settings/*` 目录页，不是业务真源。
 */

/** 设置域：个人账户 / 租户管理 / 项目（§4 A/B/C） */
export type SettingsScope = "personal" | "tenant" | "project";

/** 条目访问态：可编辑 / 只读（可见不可改）/ 不可见（无授权不出现在导航） */
export type SettingsAccess = "editable" | "readonly" | "hidden";

/** 租户管理演示角色：管理员可见可改；普通成员对整个域无授权（§4B） */
export type TenantPersona = "tenant-admin" | "tenant-member";

/** 项目设置演示角色：负责人可改；成员只读；访问者只读且成员页受限 */
export type ProjectPersona =
  | "project-lead"
  | "project-member"
  | "project-viewer";

/** 个人域恒为本人授权（§4A），无角色切换 */
export type PersonalPersona = "self";

/** 设置域任一演示角色 */
export type SettingsPersona = PersonalPersona | TenantPersona | ProjectPersona;

/** 设置导航条目（侧栏一项） */
export interface SettingsNavEntry {
  /** 稳定标识（同时是 i18n key：settings.nav.<id>） */
  id: string;
  /** 目标路由（设计目录路径） */
  to: string;
  /** 关联 Figma 帧号（如 "03"），目录页索引展示用 */
  frame?: string;
  /** 整页为 Proposed（能力未确认，§9） */
  proposed?: boolean;
}

/** 个人域导航（§4A 五类，帧 03~07） */
export const PERSONAL_NAV: SettingsNavEntry[] = [
  { id: "profile", to: "/design/settings/personal/profile", frame: "03" },
  { id: "security", to: "/design/settings/personal/security", frame: "04" },
  {
    id: "preferences",
    to: "/design/settings/personal/preferences",
    frame: "05"
  },
  {
    id: "notifications",
    to: "/design/settings/personal/notifications",
    frame: "06"
  },
  { id: "ai", to: "/design/settings/personal/ai", frame: "07" }
];

/** 租户域导航（§4B 四类；Governance 能力未确认 → 整页 Proposed，帧 09~11 + 落地页） */
export const TENANT_NAV: SettingsNavEntry[] = [
  { id: "tenantGeneral", to: "/design/settings/tenant/general", frame: "09" },
  { id: "members", to: "/design/settings/tenant/members", frame: "10" },
  {
    id: "integrations",
    to: "/design/settings/tenant/integrations",
    frame: "11"
  },
  {
    id: "governance",
    to: "/design/settings/tenant",
    frame: "08",
    proposed: true
  }
];

/** 项目域导航（§4C 五类，帧 13~17） */
export const PROJECT_NAV: SettingsNavEntry[] = [
  { id: "projectGeneral", to: "/design/settings/project/general", frame: "13" },
  { id: "projectMembers", to: "/design/settings/project/members", frame: "14" },
  {
    id: "repositories",
    to: "/design/settings/project/repositories",
    frame: "15"
  },
  {
    id: "development",
    to: "/design/settings/project/development",
    frame: "16"
  },
  { id: "projectAi", to: "/design/settings/project/ai", frame: "17" }
];

/** 各域导航归集 */
export const SETTINGS_NAV: Record<SettingsScope, SettingsNavEntry[]> = {
  personal: PERSONAL_NAV,
  tenant: TENANT_NAV,
  project: PROJECT_NAV
};

/**
 * 租户域演示访问矩阵：仅管理员能进入租户管理；
 * 普通成员对该域不可见（landing 页演示 permission denied 态，§6.9）。
 */
export const TENANT_ACCESS: Record<TenantPersona, SettingsAccess> = {
  "tenant-admin": "editable",
  "tenant-member": "hidden"
};

/**
 * 项目域演示访问矩阵（按类目）：负责人全可改；成员全只读；
 * 访问者只读且「成员」类目降为受限只读（§9 项目角色差异演示）。
 */
export const PROJECT_ACCESS: Record<
  ProjectPersona,
  Record<
    "general" | "members" | "repositories" | "development" | "ai",
    SettingsAccess
  >
> = {
  "project-lead": {
    general: "editable",
    members: "editable",
    repositories: "editable",
    development: "editable",
    ai: "editable"
  },
  "project-member": {
    general: "readonly",
    members: "readonly",
    repositories: "readonly",
    development: "readonly",
    ai: "readonly"
  },
  "project-viewer": {
    general: "readonly",
    members: "readonly",
    repositories: "readonly",
    development: "readonly",
    ai: "readonly"
  }
};

/** 演示角色的人设描述 i18n key（角色切换器副文案） */
export const PERSONA_LABEL_KEYS: Record<SettingsPersona, string> = {
  self: "settings.demo.persona.self",
  "tenant-admin": "settings.demo.persona.tenantAdmin",
  "tenant-member": "settings.demo.persona.tenantMember",
  "project-lead": "settings.demo.persona.projectLead",
  "project-member": "settings.demo.persona.projectMember",
  "project-viewer": "settings.demo.persona.projectViewer"
};

/** 演示状态（模块级单例；设计目录各页共享，跨页导航后角色保持） */
const demoState = reactive({
  tenantPersona: "tenant-admin" as TenantPersona,
  projectPersona: "project-lead" as ProjectPersona
});

/** 设计目录演示角色存储：切换当前 persona，派生域访问态 */
export function useSettingsDemo() {
  const tenantAccess = computed(() => TENANT_ACCESS[demoState.tenantPersona]);
  const projectAccess = computed(
    () => PROJECT_ACCESS[demoState.projectPersona]
  );

  function setTenantPersona(persona: TenantPersona) {
    demoState.tenantPersona = persona;
  }

  function setProjectPersona(persona: ProjectPersona) {
    demoState.projectPersona = persona;
  }

  return {
    state: demoState,
    tenantAccess,
    projectAccess,
    setTenantPersona,
    setProjectPersona
  };
}

/** 破坏性操作条目（帧 18 确认弹窗演示 + 项目常规页归档按钮共用语义） */
export interface DestructiveAction {
  /** i18n key 前缀（settings.<page>.danger.<id>.*） */
  id: string;
}

/** 成员表 fixture：姓名/邮箱为演示数据（英文免翻译），角色标签走 i18n */
export interface DemoMember {
  id: string;
  name: string;
  email: string;
  /** 角色 i18n key（settings.tenant.roles.<role> / settings.project.roles.<role>） */
  roleKey: string;
  /** 加入日期（ISO，展示层格式化） */
  joinedAt: string;
  status: "active" | "invited";
}

/** 租户成员演示数据 */
export const TENANT_MEMBERS_FIXTURE: DemoMember[] = [
  {
    id: "m-1",
    name: "Lin Han",
    email: "lin.han@example.com",
    roleKey: "settings.tenant.roles.owner",
    joinedAt: "2026-08-11",
    status: "active"
  },
  {
    id: "m-2",
    name: "Sarah Chen",
    email: "sarah.chen@example.com",
    roleKey: "settings.tenant.roles.admin",
    joinedAt: "2026-08-19",
    status: "active"
  },
  {
    id: "m-3",
    name: "David Rossi",
    email: "david.rossi@example.com",
    roleKey: "settings.tenant.roles.member",
    joinedAt: "2026-09-02",
    status: "active"
  },
  {
    id: "m-4",
    name: "Yuki Tanaka",
    email: "yuki.tanaka@example.com",
    roleKey: "settings.tenant.roles.member",
    joinedAt: "2026-10-28",
    status: "invited"
  }
];

/** 项目成员演示数据 */
export const PROJECT_MEMBERS_FIXTURE: DemoMember[] = [
  {
    id: "p-1",
    name: "Lin Han",
    email: "lin.han@example.com",
    roleKey: "settings.project.roles.lead",
    joinedAt: "2026-09-01",
    status: "active"
  },
  {
    id: "p-2",
    name: "Sarah Chen",
    email: "sarah.chen@example.com",
    roleKey: "settings.project.roles.developer",
    joinedAt: "2026-09-03",
    status: "active"
  },
  {
    id: "p-3",
    name: "Aoife Byrne",
    email: "aoife.byrne@example.com",
    roleKey: "settings.project.roles.viewer",
    joinedAt: "2026-10-14",
    status: "active"
  }
];

/** 关联仓库演示数据（帧 15：主仓库 + 关联仓库，入口指向仓库管理不复刻界面） */
export interface DemoRepository {
  id: string;
  name: string;
  path: string;
  primary: boolean;
}

export const PROJECT_REPOSITORIES_FIXTURE: DemoRepository[] = [
  { id: "r-1", name: "pm-api", path: "tooling/ai-pm", primary: true },
  {
    id: "r-2",
    name: "ai-frontend",
    path: "tooling/ai-frontend",
    primary: false
  },
  { id: "r-3", name: "docs-site", path: "tooling/docs-site", primary: false }
];

/** 活跃会话演示数据（帧 04 安全页） */
export interface DemoSession {
  id: string;
  device: string;
  location: string;
  lastActive: string;
  current: boolean;
}

export const SESSIONS_FIXTURE: DemoSession[] = [
  {
    id: "s-1",
    device: "MacBook Pro · Chrome",
    location: "Shanghai, CN",
    lastActive: "2026-11-04 15:42",
    current: true
  },
  {
    id: "s-2",
    device: "iPhone 15 · App",
    location: "Shanghai, CN",
    lastActive: "2026-11-03 22:10",
    current: false
  },
  {
    id: "s-3",
    device: "Windows 11 · Edge",
    location: "Berlin, DE",
    lastActive: "2026-10-30 09:18",
    current: false
  }
];

/** 集成提供方演示数据（帧 11 租户集成页；SSO 能力未确认 → Proposed） */
export interface DemoIntegration {
  id: string;
  /** 名称（专有名词，免翻译） */
  name: string;
  /** 描述 i18n key */
  descKey: string;
  connected: boolean;
  proposed?: boolean;
}

export const INTEGRATIONS_FIXTURE: DemoIntegration[] = [
  {
    id: "git",
    name: "Git Provider",
    descKey: "settings.tenant.integrations.git.desc",
    connected: true
  },
  {
    id: "webhook",
    name: "Notification Webhook",
    descKey: "settings.tenant.integrations.webhook.desc",
    connected: false
  },
  {
    id: "sso",
    name: "SSO / SAML",
    descKey: "settings.tenant.integrations.sso.desc",
    connected: false,
    proposed: true
  }
];
