import { i18n } from "./i18n";
import {
  routeTitleKeyByName,
  routeTitleKeyByPath
} from "./router/title-keys";

/**
 * 标题翻译的统一出口。
 *
 * 静态路由的 `meta.title` 存的是完整 i18n key（`shell.routes.*`）；DB 动态路由
 * 与菜单只有后端中文原文。key 能翻译就翻译，不能就原样返回 —— 既兼容 DB 原文，
 * 也避免 vue-i18n 对 missing key 的控制台告警。
 */

/** 页签等快照场景存「key 或原文」，渲染时再走 translateTitle，切语言自动生效 */
export function routeRawTitle(route: RouteLike): string {
  const record = route.matched?.[route.matched.length - 1];
  if (record) {
    if (record.name) {
      const byName = routeTitleKeyByName(String(record.name));
      if (byName) return byName;
    }
    const byPath = routeTitleKeyByPath(record.path);
    if (byPath) return byPath;
  }
  const title = route.meta?.title;
  return typeof title === "string" ? title : "";
}

/** key 存在则翻译，否则原样返回（DB 后端原文天然回退） */
export function translateTitle(title: unknown): string {
  if (typeof title !== "string" || !title) return "";
  return i18n.global.te(title) ? String(i18n.global.t(title)) : title;
}

export function routeTitle(route: RouteLike): string {
  return translateTitle(routeRawTitle(route));
}

/** 菜单节点标题：path 命中映射则翻译，否则回退后端原文 */
export function menuTitle(node: { path?: string; name: string }): string {
  const key = node.path ? routeTitleKeyByPath(node.path) : undefined;
  return key ? String(i18n.global.t(key)) : node.name;
}

/** 浏览器页签标题：`页面名 · AI-PM 平台` */
export function documentTitle(route: RouteLike): string {
  const title = routeTitle(route);
  const platform = String(i18n.global.t("shell.title"));
  return title ? `${title} · ${platform}` : platform;
}

/** helpers 对 vue-router 类型取最小结构，兼容 RouteLocationNormalized / RouteRecordNormalized */
interface RouteLike {
  matched?: Array<{ path: string; name?: unknown }>;
  meta?: Record<string, unknown>;
}
