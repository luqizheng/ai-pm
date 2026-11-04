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
    states: "/design/navigation/states",
    switching: "/design/navigation/switching",
    library: "/design/navigation/library"
  },
  shell: {
    states: "/design/shell/states"
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
  }
];
