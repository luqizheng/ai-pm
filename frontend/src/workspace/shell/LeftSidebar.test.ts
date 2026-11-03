// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import LeftSidebar from "./LeftSidebar.vue";
import { mountShell } from "../../shell/test-utils";
import { useWorkspaceStore } from "../store";

vi.mock("vue-router", () => ({
  useRoute: () => ({
    path: "/workspace/proj-worldmap/console",
    name: "workspace-console",
    meta: { title: "项目总控" },
    params: {},
    query: {}
  }),
  useRouter: () => ({ push: vi.fn() })
}));

// bugfix 回归：折叠后左栏只有 60px，完整文字保留会逐字竖排换行——
// 折叠态必须只留图标（分组标题/条目文字/页脚信息均隐藏）。
describe("LeftSidebar（左导航折叠态）", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.dataset.theme = "dark";
  });

  function mounted() {
    const w = mountShell(LeftSidebar);
    const store = useWorkspaceStore();
    return { w, store };
  }

  it("展开态显示分组标题与条目文字", () => {
    const { w } = mounted();
    expect(w.find(".ws-nav-group-label").exists()).toBe(true);
    expect(w.findAll(".ws-nav-item .ws-nav-label").length).toBeGreaterThan(0);
    expect(w.find("[data-test='service-ok']").exists()).toBe(true);
  });

  it("折叠态隐藏全部文字，仅剩图标与展开按钮", async () => {
    const { w, store } = mounted();
    store.sidebarCollapsed = true;
    await w.vm.$nextTick();

    expect(w.find(".ws-nav-group-label").exists()).toBe(false);
    expect(w.findAll(".ws-nav-item .ws-nav-label").length).toBe(0);
    expect(w.find("[data-test='service-ok']").exists()).toBe(false);
    // 图标条目仍在，且折叠按钮变为展开方向
    expect(w.findAll(".ws-nav-item").length).toBeGreaterThan(0);
    expect(w.find("[data-test='sidebar-toggle']").exists()).toBe(true);
  });

  it("折叠态点击条目仍可导航，点击切换按钮恢复展开", async () => {
    const { w, store } = mounted();
    store.sidebarCollapsed = true;
    await w.vm.$nextTick();

    await w.find(".ws-nav-item").trigger("click");
    expect(store.sidebarCollapsed).toBe(true);

    await w.find("[data-test='sidebar-toggle']").trigger("click");
    expect(store.sidebarCollapsed).toBe(false);
    expect(w.find(".ws-nav-group-label").exists()).toBe(true);
  });
});
