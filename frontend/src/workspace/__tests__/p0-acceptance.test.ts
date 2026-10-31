// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it, vi } from "vitest";
import { defineComponent, h } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { i18n } from "../../shell/i18n";
import { staticRoutes } from "../../shell/router/routes";
import { installAuthGuard } from "../../shell/router/guard";
import { saveSession } from "../../shell/api/token-storage";
import { THEME_STORAGE_KEY } from "../useTheme";

/** happy-dom 的 matchMedia 不随视口变化：本文件统一桩为桌面基准（断点不命中） */
const desktopMql = (query: string) => ({
  matches: false,
  media: query,
  onchange: null,
  addEventListener() {},
  removeEventListener() {},
  addListener() {},
  removeListener() {},
  dispatchEvent() {
    return false;
  }
});
vi.stubGlobal("matchMedia", desktopMql);
vi.spyOn(window, "matchMedia").mockImplementation(desktopMql as never);

/** P0 验收集成：真实路由表 + 真实组件树（spec §七） */
async function mountAt(path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: staticRoutes
  });
  const App = defineComponent({ render: () => h(RouterView) });
  const w = mount(App, {
    global: { plugins: [createPinia(), i18n, router] }
  });
  await router.push(path);
  await flushPromises();
  return w;
}

/** 需要继续导航的用例：同时暴露 router（真实参数响应性） */
async function mountRouterAt(path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: staticRoutes
  });
  const App = defineComponent({ render: () => h(RouterView) });
  const w = mount(App, {
    global: { plugins: [createPinia(), i18n, router] }
  });
  await router.push(path);
  await flushPromises();
  return { w, router };
}

/** 带 auth 守卫的挂载（M6：守卫回归用测试钉住） */
async function mountGuardedAt(path: string, opts?: { session?: boolean }) {
  localStorage.clear();
  if (opts?.session) {
    saveSession({
      accessToken: "test-access",
      refreshToken: "test-refresh",
      expiresIn: 3600,
      user: {
        id: "1",
        username: "admin",
        nickname: "管理员",
        roles: ["admin"],
        permissions: []
      }
    });
  }
  const router = createRouter({
    history: createMemoryHistory(),
    routes: staticRoutes
  });
  installAuthGuard(router);
  const App = defineComponent({ render: () => h(RouterView) });
  const w = mount(App, {
    global: { plugins: [createPinia(), i18n, router] }
  });
  await router.push(path);
  await flushPromises();
  return { w, router };
}

describe("P0 验收（总控台竖切）", () => {
  it("直达 /workspace/proj-worldmap/console 渲染全部区块 + 右栏 + 指令栏", async () => {
    const w = await mountAt("/workspace/proj-worldmap/console");
    const text = w.text();
    // 页头
    expect(text).toContain("项目总控");
    expect(text).toContain("项目运行中");
    expect(text).toContain("Iteration 2.0");
    // 简报 / 指标 / 待处理 / 动态
    expect(text).toContain("元构");
    expect(text).toContain("需要你处理");
    expect(text).toContain("地形渲染架构评审");
    expect(text).toContain("项目动态");
    // 右栏上下文
    expect(text).toContain("当前待办事项");
    expect(text).toContain("批量原型验证");
    // 指令栏
    expect(text).toContain("AI 研发负责人");
    // 左导航 IA（默认传统模式）
    expect(text).toContain("研发模块");
  });

  it("index.html 含主题防闪烁预挂载脚本", () => {
    const html = readFileSync(resolve(process.cwd(), "index.html"), "utf8");
    expect(html).toContain("aipm.theme");
    expect(html).toContain("data-theme");
  });

  it("主题切换往返并写入 localStorage", async () => {
    const w = await mountAt("/workspace/proj-worldmap/console");
    const toggle = w.find("[data-test='theme-toggle']");
    expect(document.documentElement.dataset.theme).toBe("dark");
    await toggle.trigger("click");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
    await toggle.trigger("click");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("点击待处理项 → 右栏联动", async () => {
    const w = await mountAt("/workspace/proj-worldmap/console");
    const items = w.findAll("[data-test='attention-item']");
    await items[1].trigger("click");
    const text = w.text();
    expect(text).toContain("高保数据模块验收");
    expect(text).toContain("按验收清单核对产物，通过后合入主线");
  });

  it("指令栏发送 → 回执气泡（演示链路）", async () => {
    vi.useFakeTimers();
    try {
      const w = await mountAt("/workspace/proj-worldmap/console");
      const input = w.find("[data-test='command-input']");
      expect(input.attributes("disabled")).toBeUndefined();
      await input.setValue("分析当前迭代的主要风险");
      await w.find("[data-test='command-send']").trigger("click");
      expect(w.find("[data-test='command-phase']").exists()).toBe(true);
      vi.advanceTimersByTime(800);
      await flushPromises();
      expect(w.text()).toContain("已收到指令");
    } finally {
      vi.useRealTimers();
    }
  });

  it("未知 projectId → 降级视图，不渲染演示数据", async () => {
    const w = await mountAt("/workspace/proj-other/console");
    expect(w.text()).toContain("页面不存在或无权访问");
    expect(w.text()).not.toContain("元构");
    expect(w.find("[data-test='unavailable-back']").exists()).toBe(true);
  });

  it("/workspace/:projectId 根路径重定向到 console", async () => {
    const w = await mountAt("/workspace/proj-worldmap");
    await flushPromises();
    expect(w.text()).toContain("项目总控");
    expect(w.text()).toContain("项目运行中");
  });

  it("存量页回归：/dashboard 照常渲染", async () => {
    const w = await mountAt("/dashboard");
    expect(w.find(".ui-dashboard__title").exists()).toBe(true);
    // 预览入口在
    expect(w.find("[data-test='workspace-entry']").exists()).toBe(true);
  });

  it("跨项目参数导航：项目上下文跟随新参数（M1）", async () => {
    const { w, router } = await mountRouterAt("/workspace/proj-other/console");
    expect(w.text()).toContain("页面不存在或无权访问");
    // 同一壳实例（同路由记录复用），上下文必须跟随新参数，而不是停留在首次挂载
    await router.push("/workspace/proj-worldmap/console");
    await flushPromises();
    const input = w.find("[data-test='command-input']");
    expect(input.attributes("disabled")).toBeUndefined();
    await input.setValue("导航后的指令");
    await w.find("[data-test='command-send']").trigger("click");
    expect(w.text()).toContain("导航后的指令");
  });

  it("非总控台工作区页（ComingSoon）指令栏有项目上下文（M4）", async () => {
    const { w } = await mountRouterAt("/workspace/proj-worldmap/team");
    const input = w.find("[data-test='command-input']");
    expect(input.attributes("disabled")).toBeUndefined();
    expect(w.text()).not.toContain("请先选择项目");
  });

  it("降级页面包屑不冒充演示项目（M3）", async () => {
    const { w } = await mountRouterAt("/workspace/proj-other/console");
    expect(w.text()).toContain("proj-other");
    expect(w.text()).not.toContain("世界地图编辑器");
  });

  it("守卫：未登录访问工作区 → 重定向登录页（M6）", async () => {
    const { router } = await mountGuardedAt("/workspace/proj-worldmap/console");
    await flushPromises();
    expect(router.currentRoute.value.name).toBe("login");
  });

  it("守卫：已登录访问工作区 → 正常渲染（M6）", async () => {
    const { w } = await mountGuardedAt("/workspace/proj-worldmap/console", {
      session: true
    });
    await flushPromises();
    expect(w.text()).toContain("项目总控");
    expect(w.text()).toContain("项目运行中");
  });

  it("全部工作区页签渲染对应页面（mock 数据，非建设中）（/goal 全页签）", async () => {
    const { w, router } = await mountRouterAt("/workspace/proj-worldmap/team");
    const cases: [string, string][] = [
      ["execution", "AI 执行中心"],
      ["team", "AI 团队"],
      ["goals", "目标计划"],
      ["activity", "项目动态"],
      ["requirements", "需求管理"],
      ["decisions", "决策中心"],
      ["knowledge", "项目知识"],
      ["artifacts", "产物验收"]
    ];
    for (const [slug, heading] of cases) {
      await router.push(`/workspace/proj-worldmap/${slug}`);
      await flushPromises();
      expect(w.text(), `${slug} 应渲染「${heading}」`).toContain(heading);
      expect(w.text(), `${slug} 不应落在建设中占位`).not.toContain(
        "该页面正在迁移"
      );
    }
    // mock 数据随页可见：目标计划有里程碑，产物验收有高保数据模块
    await router.push("/workspace/proj-worldmap/goals");
    await flushPromises();
    expect(w.text()).toContain("架构评审通过");
    await router.push("/workspace/proj-worldmap/artifacts");
    await flushPromises();
    expect(w.text()).toContain("高保数据模块");
  });
});
