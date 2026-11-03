// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { i18n, setLocale } from "./i18n";
import { documentTitle, menuTitle, routeTitle, translateTitle } from "./i18n-helpers";

/** 最小路由对象：helpers 只依赖 matched / meta */
function routeOf(path: string, metaTitle?: string, name?: string) {
  return {
    matched: [{ path, name }],
    meta: metaTitle ? { title: metaTitle } : {}
  };
}

describe("i18n-helpers", () => {
  it("静态路由标题按 key 翻译，zh 与 en 都命中", () => {
    setLocale("zh-CN");
    expect(routeTitle(routeOf("/dashboard", "shell.routes.dashboard", "dashboard"))).toBe("概览");
    expect(routeTitle(routeOf("/login", "shell.routes.login", "login"))).toBe("登录");

    setLocale("en");
    expect(routeTitle(routeOf("/dashboard", "shell.routes.dashboard", "dashboard"))).toBe("Overview");
    expect(routeTitle(routeOf("/login", "shell.routes.login", "login"))).toBe("Sign in");
    setLocale("zh-CN");
  });

  it("后端原文标题原样返回（DB 动态路由回退，不产生 missing key）", () => {
    setLocale("en");
    expect(routeTitle(routeOf("/pm/xxx", "审批流转"))).toBe("审批流转");
    setLocale("zh-CN");
  });

  it("DB 菜单按 path 映射翻译；未登记路径回退后端原文", () => {
    setLocale("en");
    expect(menuTitle({ path: "/system/user", name: "用户管理" })).toBe("Users");
    expect(menuTitle({ path: "/audit/login-log", name: "登录日志" })).toBe("Login Logs");
    // 本地 mock 菜单路径同样覆盖
    expect(menuTitle({ path: "/projects", name: "项目" })).toBe("Projects");
    // 未登记路径：回退后端原文
    expect(menuTitle({ path: "/unknown/path", name: "未知菜单" })).toBe("未知菜单");
    setLocale("zh-CN");
  });

  it("translateTitle 对非 key 字符串原样返回", () => {
    expect(translateTitle("")).toBe("");
    expect(translateTitle(undefined)).toBe("");
    expect(translateTitle(42)).toBe("");
    expect(translateTitle("shell.routes.dashboard")).toBe("概览");
  });

  it("浏览器标题为「页面名 · 平台名」，随语言切换", () => {
    setLocale("zh-CN");
    expect(documentTitle(routeOf("/dashboard", "shell.routes.dashboard", "dashboard"))).toBe(
      "概览 · AI-PM 平台"
    );
    setLocale("en");
    expect(documentTitle(routeOf("/dashboard", "shell.routes.dashboard", "dashboard"))).toBe(
      "Overview · AI-PM Console"
    );
    setLocale("zh-CN");
  });

  it("i18n 实例处于 zh-CN（setLocale 已持久化切换）", () => {
    expect(i18n.global.locale.value).toBe("zh-CN");
  });
});
