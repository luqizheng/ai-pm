// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import ContextSidebar from "./ContextSidebar.vue";
import { mountShell } from "../../shell/test-utils";
import { useWorkspaceStore } from "../store";
import { attentionItems, contextAdvice } from "../fixtures";

vi.mock("vue-router", () => ({
  useRoute: () => ({ path: "/workspace/proj-worldmap/console", params: {}, query: {} }),
  useRouter: () => ({ push: vi.fn() })
}));

describe("ContextSidebar（右栏上下文）", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.dataset.theme = "dark";
  });

  function mounted() {
    const w = mountShell(ContextSidebar);
    const store = useWorkspaceStore();
    return { w, store };
  }

  it("未选中时默认展示首条待办与右栏 AI 建议", () => {
    const { w } = mounted();
    const text = w.text();
    expect(text).toContain("当前待办事项");
    expect(text).toContain(attentionItems[0].title);
    expect(text).toContain(contextAdvice);
  });

  it("选中变更后右栏标题与建议跟随", async () => {
    const { w, store } = mounted();
    store.selectAttention(attentionItems[2].id);
    await w.vm.$nextTick();
    const text = w.text();
    expect(text).toContain(attentionItems[2].title);
    expect(text).toContain(attentionItems[2].advice);
    expect(text).not.toContain(contextAdvice);
  });

  it("展示当前对象与两个占位区", () => {
    const { w } = mounted();
    const text = w.text();
    expect(text).toContain("批量原型验证");
    expect(text).toContain("补充证据");
  });

  it("点击折叠图标调 toggleContext", async () => {
    const { w, store } = mounted();
    await w.find("[data-test='context-collapse']").trigger("click");
    expect(store.contextOpen).toBe(false);
  });
});
