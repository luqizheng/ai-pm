import type { RouteRecordRaw } from "vue-router";

/**
 * 设计系统目录路由 —— Figma「AI R&D Workspace」设计稿的可浏览实现。
 *
 * 权威来源：Figma `hNlGqLEMQpOVCZ0o5u3DkT`
 *   - 画布「13 — Component Library」(13:37296) —— DS 组件族
 *   - 画布「Page 1」 —— 02 Authentication / 03 Application Shell / 01~08 Navigation 板
 *
 * 与应用壳路由分离：目录页是无导航的全屏页（`meta.layout = "design"`），
 * 由 App.vue 按 `route.meta.layout` 切换外壳 —— 与认证页（`layout = "auth"`）同机制。
 */

/** 设计系统路径常量（视图内跳转用，避免散落字面量） */
export const DESIGN_PATHS = {
  index: "/design",
  tokens: "/design/tokens",
  components: {
    buttons: "/design/components/buttons",
    forms: "/design/components/forms",
    navigation: "/design/components/navigation",
    overlays: "/design/components/overlays",
    feedback: "/design/components/feedback",
    data: "/design/components/data",
    ai: "/design/components/ai"
  },
  navigation: {
    global: "/design/navigation/global",
    contexts: "/design/navigation/contexts",
    switcher: "/design/navigation/switcher",
    states: "/design/navigation/states",
    switching: "/design/navigation/switching",
    library: "/design/navigation/library"
  },
  shell: {
    states: "/design/shell/states"
  },
  /** Settings Framework（19 帧，Settings Framework V1.0 §8） */
  settings: {
    architecture: "/design/settings/architecture",
    personal: "/design/settings/personal",
    personalProfile: "/design/settings/personal/profile",
    personalSecurity: "/design/settings/personal/security",
    personalPreferences: "/design/settings/personal/preferences",
    personalNotifications: "/design/settings/personal/notifications",
    personalAi: "/design/settings/personal/ai",
    tenant: "/design/settings/tenant",
    tenantGeneral: "/design/settings/tenant/general",
    tenantMembers: "/design/settings/tenant/members",
    tenantIntegrations: "/design/settings/tenant/integrations",
    project: "/design/settings/project",
    projectGeneral: "/design/settings/project/general",
    projectMembers: "/design/settings/project/members",
    projectRepositories: "/design/settings/project/repositories",
    projectDevelopment: "/design/settings/project/development",
    projectAi: "/design/settings/project/ai",
    states: "/design/settings/states",
    components: "/design/settings/components"
  }
} as const;

/** 设计系统路由记录（全部为无外壳全屏页） */
export const designRoutes: RouteRecordRaw[] = [
  {
    path: DESIGN_PATHS.index,
    name: "design-index",
    component: () => import("@/views/design/DesignIndexView.vue"),
    meta: { layout: "design", title: "design.index.title" }
  },
  {
    path: DESIGN_PATHS.tokens,
    name: "design-tokens",
    component: () => import("@/views/design/DesignTokensView.vue"),
    meta: { layout: "design", title: "design.tokens.title" }
  },
  {
    path: DESIGN_PATHS.components.buttons,
    name: "design-components-buttons",
    component: () => import("@/views/design/components/DesignButtonsView.vue"),
    meta: { layout: "design", title: "design.components.buttons.title" }
  },
  {
    path: DESIGN_PATHS.components.forms,
    name: "design-components-forms",
    component: () => import("@/views/design/components/DesignFormsView.vue"),
    meta: { layout: "design", title: "design.components.forms.title" }
  },
  {
    path: DESIGN_PATHS.components.navigation,
    name: "design-components-navigation",
    component: () =>
      import("@/views/design/components/DesignNavigationView.vue"),
    meta: { layout: "design", title: "design.components.navigation.title" }
  },
  {
    path: DESIGN_PATHS.components.overlays,
    name: "design-components-overlays",
    component: () => import("@/views/design/components/DesignOverlaysView.vue"),
    meta: { layout: "design", title: "design.components.overlays.title" }
  },
  {
    path: DESIGN_PATHS.components.feedback,
    name: "design-components-feedback",
    component: () => import("@/views/design/components/DesignFeedbackView.vue"),
    meta: { layout: "design", title: "design.components.feedback.title" }
  },
  {
    path: DESIGN_PATHS.components.data,
    name: "design-components-data",
    component: () => import("@/views/design/components/DesignDataView.vue"),
    meta: { layout: "design", title: "design.components.data.title" }
  },
  {
    path: DESIGN_PATHS.components.ai,
    name: "design-components-ai",
    component: () => import("@/views/design/components/DesignAiView.vue"),
    meta: { layout: "design", title: "design.components.ai.title" }
  },
  {
    path: DESIGN_PATHS.navigation.global,
    name: "design-navigation-global",
    component: () =>
      import("@/views/design/navigation/DesignNavGlobalView.vue"),
    meta: { layout: "design", title: "design.navigation.global.title" }
  },
  {
    path: DESIGN_PATHS.navigation.contexts,
    name: "design-navigation-contexts",
    component: () =>
      import("@/views/design/navigation/DesignNavContextsView.vue"),
    meta: { layout: "design", title: "design.navigation.contexts.title" }
  },
  {
    path: DESIGN_PATHS.navigation.switcher,
    name: "design-navigation-switcher",
    component: () =>
      import("@/views/design/navigation/DesignNavSwitcherView.vue"),
    meta: { layout: "design", title: "design.navigation.switcher.title" }
  },
  {
    path: DESIGN_PATHS.navigation.states,
    name: "design-navigation-states",
    component: () =>
      import("@/views/design/navigation/DesignNavStatesView.vue"),
    meta: { layout: "design", title: "design.navigation.states.title" }
  },
  {
    path: DESIGN_PATHS.navigation.switching,
    name: "design-navigation-switching",
    component: () =>
      import("@/views/design/navigation/DesignNavSwitchingView.vue"),
    meta: { layout: "design", title: "design.navigation.switching.title" }
  },
  {
    path: DESIGN_PATHS.navigation.library,
    name: "design-navigation-library",
    component: () =>
      import("@/views/design/navigation/DesignNavLibraryView.vue"),
    meta: { layout: "design", title: "design.navigation.library.title" }
  },
  {
    path: DESIGN_PATHS.shell.states,
    name: "design-shell-states",
    component: () => import("@/views/design/shell/DesignShellStatesView.vue"),
    meta: { layout: "design", title: "design.shell.states.title" }
  },

  // ===== Settings Framework（19 帧）=====
  {
    path: DESIGN_PATHS.settings.architecture,
    name: "design-settings-architecture",
    component: () =>
      import("@/views/design/settings/SettingsArchitectureView.vue"),
    meta: { layout: "design", title: "settings.architecture.title" }
  },
  {
    path: DESIGN_PATHS.settings.personal,
    name: "design-settings-personal",
    component: () => import("@/views/design/settings/SettingsPersonalView.vue"),
    meta: { layout: "design", title: "settings.personal.landing.title" }
  },
  {
    path: DESIGN_PATHS.settings.personalProfile,
    name: "design-settings-personal-profile",
    component: () =>
      import("@/views/design/settings/SettingsPersonalProfileView.vue"),
    meta: { layout: "design", title: "settings.nav.profile" }
  },
  {
    path: DESIGN_PATHS.settings.personalSecurity,
    name: "design-settings-personal-security",
    component: () =>
      import("@/views/design/settings/SettingsPersonalSecurityView.vue"),
    meta: { layout: "design", title: "settings.nav.security" }
  },
  {
    path: DESIGN_PATHS.settings.personalPreferences,
    name: "design-settings-personal-preferences",
    component: () =>
      import("@/views/design/settings/SettingsPersonalPreferencesView.vue"),
    meta: { layout: "design", title: "settings.nav.preferences" }
  },
  {
    path: DESIGN_PATHS.settings.personalNotifications,
    name: "design-settings-personal-notifications",
    component: () =>
      import("@/views/design/settings/SettingsPersonalNotificationsView.vue"),
    meta: { layout: "design", title: "settings.nav.notifications" }
  },
  {
    path: DESIGN_PATHS.settings.personalAi,
    name: "design-settings-personal-ai",
    component: () =>
      import("@/views/design/settings/SettingsPersonalAiView.vue"),
    meta: { layout: "design", title: "settings.nav.ai" }
  },
  {
    path: DESIGN_PATHS.settings.tenant,
    name: "design-settings-tenant",
    component: () => import("@/views/design/settings/SettingsTenantView.vue"),
    meta: { layout: "design", title: "settings.tenant.landing.title" }
  },
  {
    path: DESIGN_PATHS.settings.tenantGeneral,
    name: "design-settings-tenant-general",
    component: () =>
      import("@/views/design/settings/SettingsTenantGeneralView.vue"),
    meta: { layout: "design", title: "settings.nav.tenantGeneral" }
  },
  {
    path: DESIGN_PATHS.settings.tenantMembers,
    name: "design-settings-tenant-members",
    component: () =>
      import("@/views/design/settings/SettingsTenantMembersView.vue"),
    meta: { layout: "design", title: "settings.nav.members" }
  },
  {
    path: DESIGN_PATHS.settings.tenantIntegrations,
    name: "design-settings-tenant-integrations",
    component: () =>
      import("@/views/design/settings/SettingsTenantIntegrationsView.vue"),
    meta: { layout: "design", title: "settings.nav.integrations" }
  },
  {
    path: DESIGN_PATHS.settings.project,
    name: "design-settings-project",
    component: () => import("@/views/design/settings/SettingsProjectView.vue"),
    meta: { layout: "design", title: "settings.project.landing.title" }
  },
  {
    path: DESIGN_PATHS.settings.projectGeneral,
    name: "design-settings-project-general",
    component: () =>
      import("@/views/design/settings/SettingsProjectGeneralView.vue"),
    meta: { layout: "design", title: "settings.nav.projectGeneral" }
  },
  {
    path: DESIGN_PATHS.settings.projectMembers,
    name: "design-settings-project-members",
    component: () =>
      import("@/views/design/settings/SettingsProjectMembersView.vue"),
    meta: { layout: "design", title: "settings.nav.projectMembers" }
  },
  {
    path: DESIGN_PATHS.settings.projectRepositories,
    name: "design-settings-project-repositories",
    component: () =>
      import("@/views/design/settings/SettingsProjectRepositoriesView.vue"),
    meta: { layout: "design", title: "settings.nav.repositories" }
  },
  {
    path: DESIGN_PATHS.settings.projectDevelopment,
    name: "design-settings-project-development",
    component: () =>
      import("@/views/design/settings/SettingsProjectDevelopmentView.vue"),
    meta: { layout: "design", title: "settings.nav.development" }
  },
  {
    path: DESIGN_PATHS.settings.projectAi,
    name: "design-settings-project-ai",
    component: () =>
      import("@/views/design/settings/SettingsProjectAiView.vue"),
    meta: { layout: "design", title: "settings.nav.projectAi" }
  },
  {
    path: DESIGN_PATHS.settings.states,
    name: "design-settings-states",
    component: () => import("@/views/design/settings/SettingsStatesView.vue"),
    meta: { layout: "design", title: "settings.states.title" }
  },
  {
    path: DESIGN_PATHS.settings.components,
    name: "design-settings-components",
    component: () =>
      import("@/views/design/settings/SettingsComponentsView.vue"),
    meta: { layout: "design", title: "settings.components.title" }
  }
];
