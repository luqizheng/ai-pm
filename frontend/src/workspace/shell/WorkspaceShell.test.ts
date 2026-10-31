// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import WorkspaceShell from "./WorkspaceShell.vue";
import { mountShell } from "../../shell/test-utils";
import { THEME_STORAGE_KEY } from "../useTheme";

type MediaListener = (e: { matches: boolean }) => void;

/** happy-dom 的 matchMedia 不会触发 change，测试用桩接管 */
function stubMatchMedia(initial: Record<string, boolean>) {
  const listeners = new Map<string, MediaListener[]>();
  const mql = (query: string) => ({
    matches: initial[query] ?? false,
    media: query,
    onchange: null,
    addEventListener(_t: string, cb: MediaListener) {
      const list = listeners.get(query) ?? [];
      list.push(cb);
      listeners.set(query, list);
    },
    removeEventListener() {},
    addListener() {},
    removeListener() {},
    dispatchEvent() {
      return false;
    }
  });
  vi.stubGlobal("matchMedia", mql);
  vi.spyOn(window, "matchMedia").mockImplementation(mql as never);
  return listeners;
}

function fire(listeners: Map<string, MediaListener[]>, query: string) {
  for (const cb of listeners.get(query) ?? []) cb({ matches: true });
}

const push = vi.fn();
vi.mock("vue-router", () => ({
  useRoute: () => ({
    path: "/workspace/proj-worldmap/console",
    params: { projectId: "proj-worldmap" },
    meta: { title: "项目总控" },
    query: {}
  }),
  useRouter: () => ({ push }),
  RouterView: { template: "<div class='stub-view' />" }
}));

describe("WorkspaceShell（V1.1 三栏壳）", () => {
  beforeEach(() => {
    push.mockClear();
    localStorage.clear();
    localStorage.setItem(THEME_STORAGE_KEY, "dark");
    document.documentElement.dataset.theme = "dark";
    stubMatchMedia({});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  function mountShellWithSlots() {
    return mountShell(WorkspaceShell, {
      props: { project: "世界地图编辑器", page: "项目总控" },
      slots: {
        main: "<div data-test='main-slot' />",
        context: "<div data-test='ctx-slot' />"
      }
    });
  }

  it("渲染 TopBar / 左导航 / 主区 / 右栏四区", () => {
    const w = mountShellWithSlots();
    expect(w.find(".workspace-topbar").exists()).toBe(true);
    expect(w.find(".workspace-leftnav").exists()).toBe(true);
    expect(w.find("[data-test='main-slot']").exists()).toBe(true);
    expect(w.find("[data-test='ctx-slot']").exists()).toBe(true);
  });

  it("左导航默认渲染传统模式 9 项（工作空间组 + 研发模块组）", () => {
    const w = mountShellWithSlots();
    const nav = w.find(".workspace-leftnav").text();
    for (const label of [
      "项目总控",
      "AI 团队",
      "目标计划",
      "项目动态",
      "需求管理",
      "决策中心",
      "项目知识",
      "产物验收",
      "模型配置"
    ]) {
      expect(nav).toContain(label);
    }
    expect(nav).toContain("工作空间");
    expect(nav).toContain("研发模块");
    // 「传统视图」组 / 概览入口已随裁决移除
    expect(nav).not.toContain("传统视图");
    expect(nav).not.toContain("概览");
  });

  it("切到 AI 模式后左导航渲染 V1.2 五项，被裁项隐藏", async () => {
    const w = mountShellWithSlots();
    await w.find("[data-test='mode-ai']").trigger("click");
    const nav = w.find(".workspace-leftnav").text();
    for (const label of [
      "目标与计划",
      "AI 执行中心",
      "决策中心",
      "项目认知",
      "产物验收"
    ]) {
      expect(nav).toContain(label);
    }
    for (const label of [
      "项目总控",
      "AI 团队",
      "项目动态",
      "模型配置",
      "需求管理",
      "项目知识"
    ]) {
      expect(nav).not.toContain(label);
    }
  });

  it("模式来回切换：AI → 传统，左栏恢复 9 项", async () => {
    const w = mountShellWithSlots();
    await w.find("[data-test='mode-ai']").trigger("click");
    expect(w.find(".workspace-leftnav").text()).toContain("AI 执行中心");
    await w.find("[data-test='mode-toggle']").trigger("click");
    const nav = w.find(".workspace-leftnav").text();
    expect(nav).toContain("项目总控");
    expect(nav).toContain("模型配置");
    expect(nav).not.toContain("AI 执行中心");
  });

  it("点击折叠按钮后壳根进入 collapsed 形态", async () => {
    const w = mountShellWithSlots();
    expect(w.find(".workspace-shell").attributes("data-sidebar")).toBe(
      "expanded"
    );
    await w.find("[data-test='sidebar-toggle']").trigger("click");
    expect(w.find(".workspace-shell").attributes("data-sidebar")).toBe(
      "collapsed"
    );
  });

  it("窄断点（≤1280px）自动收起右栏", async () => {
    const listeners = stubMatchMedia({});
    const w = mountShellWithSlots();
    expect(w.find(".workspace-shell").attributes("data-context")).toBe("open");
    fire(listeners, "(max-width: 1280px)");
    await nextTick();
    expect(w.find(".workspace-shell").attributes("data-context")).toBe(
      "closed"
    );
  });

  it("右栏收起后 TopBar 出现重开按钮并恢复右栏（终审 I1）", async () => {
    // 不提供 context 插槽 → fallback 渲染 ContextSidebar（其内有折叠按钮）
    const w = mountShell(WorkspaceShell, {
      props: { project: "世界地图编辑器", page: "项目总控" },
      slots: { main: "<div data-test='main-slot' />" }
    });
    expect(
      w.find("[data-test='context-reopen']").exists(),
      "右栏打开时不应出现重开按钮"
    ).toBe(false);
    await w.find("[data-test='context-collapse']").trigger("click");
    expect(w.find(".workspace-shell").attributes("data-context")).toBe(
      "closed"
    );
    const reopen = w.find("[data-test='context-reopen']");
    expect(reopen.exists()).toBe(true);
    await reopen.trigger("click");
    expect(w.find(".workspace-shell").attributes("data-context")).toBe("open");
    expect(
      w.find("[data-test='context-reopen']").exists(),
      "右栏恢复后重开按钮消失"
    ).toBe(false);
  });

  it("窗口从窄断点变宽后右栏自动恢复（终审 I1）", async () => {
    const listeners = stubMatchMedia({});
    const w = mountShellWithSlots();
    fire(listeners, "(max-width: 1280px)");
    await nextTick();
    expect(w.find(".workspace-shell").attributes("data-context")).toBe(
      "closed"
    );
    for (const cb of listeners.get("(max-width: 1280px)") ?? []) {
      cb({ matches: false });
    }
    await nextTick();
    expect(w.find(".workspace-shell").attributes("data-context")).toBe("open");
  });

  it("紧凑断点（≤1024px）左栏折叠至图标栏", async () => {
    const listeners = stubMatchMedia({});
    const w = mountShellWithSlots();
    fire(listeners, "(max-width: 1024px)");
    await nextTick();
    expect(w.find(".workspace-shell").attributes("data-sidebar")).toBe(
      "collapsed"
    );
  });

  it("面包屑展示项目与页面文案", () => {
    const w = mountShellWithSlots();
    const text = w.find(".workspace-topbar").text();
    expect(text).toContain("世界地图编辑器");
    expect(text).toContain("项目总控");
  });

  it("TopBar 有 Atlas R&D 品牌标识（原型对齐）", () => {
    const w = mountShellWithSlots();
    expect(w.find("[data-test='brand']").exists()).toBe(true);
    expect(w.find(".workspace-topbar").text()).toContain("Atlas R&D");
  });

  it("左栏底部展示当前迭代（原型对齐）", () => {
    const w = mountShellWithSlots();
    expect(w.text()).toContain("当前迭代");
    expect(w.text()).toContain("Iteration 2.0");
  });

  it("TopBar 主题切换按钮往返深浅主题", async () => {
    const w = mountShellWithSlots();
    expect(document.documentElement.dataset.theme).toBe("dark");
    const toggle = w.find("[data-test='theme-toggle']");
    expect(toggle.exists()).toBe(true);
    await toggle.trigger("click");
    expect(document.documentElement.dataset.theme).toBe("light");
    await toggle.trigger("click");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("左下角服务状态为绿点 + 服务正常", () => {
    const w = mountShellWithSlots();
    expect(w.find("[data-test='service-ok']").text()).toContain("服务正常");
    expect(w.find("[data-test='service-dot']").exists()).toBe(true);
  });

  it("模式切换在壳内换导航集，不跳 /dashboard；单模式页回退到目标默认页", async () => {
    const w = mountShellWithSlots();
    // 当前页 console 仅存在于传统模式 → 切 AI 回退到 AI 默认页 goals
    await w.find("[data-test='mode-ai']").trigger("click");
    expect(push).toHaveBeenCalledWith("/workspace/proj-worldmap/goals");
    expect(push).not.toHaveBeenCalledWith("/dashboard");
  });

  it("模式切换为带文字的分段控件（用户反馈：图标按钮不可发现）", () => {
    const w = mountShellWithSlots();
    const switchGroup = w.find("[data-test='mode-switch']");
    expect(switchGroup.exists()).toBe(true);
    const text = switchGroup.text();
    expect(text).toContain("AI 模式");
    expect(text).toContain("传统模式");
    // 默认传统模式：当前段为「传统模式」，目标段「AI 模式」可点
    const current = switchGroup.find("[aria-pressed='true']");
    expect(current.exists()).toBe(true);
    expect(current.text()).toContain("传统模式");
    const target = switchGroup.find("[aria-pressed='false']");
    expect(target.exists()).toBe(true);
    expect(target.text()).toContain("AI 模式");
  });

  it("右栏收起后重开按钮带文字（用户反馈：图标不可发现）", async () => {
    const w = mountShell(WorkspaceShell, {
      props: { project: "世界地图编辑器", page: "项目总控" },
      slots: { main: "<div data-test='main-slot' />" }
    });
    await w.find("[data-test='context-collapse']").trigger("click");
    const reopen = w.find("[data-test='context-reopen']");
    expect(reopen.exists()).toBe(true);
    expect(reopen.text()).toContain("显示上下文");
    await reopen.trigger("click");
    expect(w.find(".workspace-shell").attributes("data-context")).toBe("open");
  });
});
