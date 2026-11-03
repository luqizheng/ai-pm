import type { RouteRecordRaw } from "vue-router";
import { staticRoutes } from "./routes";

/**
 * 路由/菜单标题的 i18n key 映射。
 *
 * 静态路由（routes.ts）的 `meta.title` 直接存完整 key（如 `shell.routes.dashboard`），
 * 这里自动推导出 path 模式与 name → key 的映射；DB 菜单（prue get-async-routes）
 * 的 title 是后端原文，前端按 path 命中下表翻译，未命中的回退显示原文。
 *
 * DB 菜单是两套固定种子：线上 seed（upstream-menu.json）与本地 mock
 * （src/api/mock/fixtures.ts 的 mockAsyncRoutes），逐条登记，两套都覆盖。
 */
const MENU_PATH_TITLE_KEYS: Record<string, string> = {
  // ── 线上 seed（upstream-menu.json） ─────────────────────────────────
  "/dashboard": "shell.routes.dashboard",
  "/system": "shell.routes.system",
  "/audit": "shell.routes.audit",
  "/system/user": "shell.routes.systemUser",
  "/system/role": "shell.routes.systemRole",
  "/system/permission": "shell.routes.systemPermission",
  "/system/dept": "shell.routes.systemDept",
  "/system/menu": "shell.routes.systemMenu",
  "/system/session": "shell.routes.systemSession",
  "/system/token": "shell.routes.systemToken",
  "/audit/login-log": "shell.routes.auditLoginLog",
  "/audit/operation-log": "shell.routes.auditOperationLog",
  "/notification/announcement": "shell.routes.notificationAnnouncement",
  "/file": "shell.routes.file",
  "/config/item": "shell.routes.config",

  // ── 本地 mock（mockAsyncRoutes） ────────────────────────────────────
  "/overview": "shell.routes.dashboard",
  "/delivery": "shell.routes.delivery",
  "/projects": "shell.routes.projects",
  "/requirements": "shell.routes.requirements",
  "/tasks": "shell.routes.tasks",
  "/governance": "shell.routes.governance",
  "/approvals": "shell.routes.approvals",
  "/pm-settings": "shell.routes.settings",
  "/system/users": "shell.routes.systemUser",
  "/system/roles": "shell.routes.systemRole",
  "/system/menus": "shell.routes.systemMenu",
  "/system/sessions": "shell.routes.systemSession",
  "/audit-center": "shell.routes.audit",
  "/audit/login-logs": "shell.routes.auditLoginLog",
  "/audit/operation-logs": "shell.routes.auditOperationLog",
  "/notifications": "shell.routes.notificationAnnouncement",
  "/inbox": "shell.routes.inbox",
  "/config-items": "shell.routes.config",
  "/files": "shell.routes.file"
};

function joinPath(prefix: string, child: string): string {
  if (child.startsWith("/")) return child;
  return prefix.endsWith("/") ? `${prefix}${child}` : `${prefix}/${child}`;
}

function walkStatic(
  routes: RouteRecordRaw[],
  prefix: string,
  byPath: Map<string, string>,
  byName: Map<string, string>
) {
  for (const route of routes) {
    const path = joinPath(prefix, route.path);
    const title = route.meta?.title;
    if (typeof title === "string" && title) {
      byPath.set(path, title);
      if (route.name) byName.set(String(route.name), title);
    }
    if (route.children?.length)
      walkStatic(route.children, path, byPath, byName);
  }
}

interface TitleMaps {
  byPath: Map<string, string>;
  byName: Map<string, string>;
}

let cached: TitleMaps | null = null;

/**
 * 惰性构建映射（**不能**在模块求值期做）。
 *
 * 存在循环依赖：routes.ts → AppShell.vue → i18n-helpers.ts → 本模块 → routes.ts。
 * 模块求值期 routes.ts 还没执行到 `export const staticRoutes`，那时取到的是
 * undefined。改在首次调用（渲染期，routes.ts 必然已就绪）时构建并缓存。
 */
function maps(): TitleMaps {
  if (cached) return cached;

  const byPath = new Map<string, string>();
  const byName = new Map<string, string>();
  // staticRoutes 未就绪时（理论上不会发生）只给 DB 菜单映射，且不缓存
  const ready = Array.isArray(staticRoutes);
  if (ready) walkStatic(staticRoutes, "", byPath, byName);

  for (const [path, key] of Object.entries(MENU_PATH_TITLE_KEYS)) {
    if (!byPath.has(path)) byPath.set(path, key);
  }

  const result = { byPath, byName };
  if (ready) cached = result;
  return result;
}

/** path 模式（:params 保留）→ 标题 key；菜单节点按原始 path 直接命中 */
export function routeTitleKeyByPath(path: string): string | undefined {
  return maps().byPath.get(path);
}

/** 路由 name → 标题 key（静态路由均有 name，动态菜单路由是 menu-* 前缀不会命中） */
export function routeTitleKeyByName(name: string): string | undefined {
  return maps().byName.get(name);
}
