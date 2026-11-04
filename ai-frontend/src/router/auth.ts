import type { RouteRecordRaw } from "vue-router";

/**
 * 认证路由 —— Figma「02 — Authentication」九帧的路由表。
 *
 * 与应用壳路由分离：认证页无导航外壳（App.vue 按 route.meta.layout 切换），
 * 登录成功后由认证页跳回工作区。
 */

/** 认证页路径常量（视图内跳转用，避免散落字面量） */
export const AUTH_PATHS = {
  login: "/auth/login",
  forgotPassword: "/auth/forgot-password",
  resetPassword: "/auth/reset-password",
  invitation: "/auth/invitation",
  signingIn: "/auth/signing-in",
  sessionExpired: "/auth/session-expired",
  unauthorized: "/auth/unauthorized",
  patterns: "/auth/patterns"
} as const;

/** 认证路由记录（meta.layout = "auth" 表示走认证页外壳） */
export const authRoutes: RouteRecordRaw[] = [
  {
    path: AUTH_PATHS.login,
    name: "auth-login",
    component: () => import("@/views/auth/LoginView.vue"),
    meta: { layout: "auth", title: "auth.login.title" }
  },
  {
    path: AUTH_PATHS.forgotPassword,
    name: "auth-forgot-password",
    component: () => import("@/views/auth/ForgotPasswordView.vue"),
    meta: { layout: "auth", title: "auth.forgot.title" }
  },
  {
    path: AUTH_PATHS.resetPassword,
    name: "auth-reset-password",
    component: () => import("@/views/auth/ResetPasswordView.vue"),
    meta: { layout: "auth", title: "auth.reset.title" }
  },
  {
    path: AUTH_PATHS.invitation,
    name: "auth-invitation",
    component: () => import("@/views/auth/InvitationView.vue"),
    meta: { layout: "auth", title: "auth.invitation.title" }
  },
  {
    path: AUTH_PATHS.signingIn,
    name: "auth-signing-in",
    component: () => import("@/views/auth/AuthLoadingView.vue"),
    meta: { layout: "auth", title: "auth.loading.title" }
  },
  {
    path: AUTH_PATHS.sessionExpired,
    name: "auth-session-expired",
    component: () => import("@/views/auth/SessionExpiredView.vue"),
    meta: { layout: "auth", title: "auth.sessionExpired.title" }
  },
  {
    path: AUTH_PATHS.unauthorized,
    name: "auth-unauthorized",
    component: () => import("@/views/auth/UnauthorizedView.vue"),
    meta: { layout: "auth", title: "auth.unauthorized.title" }
  },
  {
    path: AUTH_PATHS.patterns,
    name: "auth-patterns",
    component: () => import("@/views/auth/AuthPatternsView.vue"),
    // 整屏参照页：不套认证页外壳（自带头部与底注），也不套应用壳
    meta: { layout: "bare", title: "auth.patterns.title" }
  }
];
