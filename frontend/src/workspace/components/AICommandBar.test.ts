// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import AICommandBar from "./AICommandBar.vue";
import { mountShell } from "../../shell/test-utils";
import { useWorkspaceStore } from "../store";

vi.mock("vue-router", () => ({
  useRoute: () => ({
    path: "/workspace/proj-worldmap/console",
    params: {},
    query: {}
  }),
  useRouter: () => ({ push: vi.fn() })
}));

describe("AICommandBar（指令栏）", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    localStorage.clear();
    document.documentElement.dataset.theme = "dark";
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  async function mounted() {
    const w = mountShell(AICommandBar);
    const store = useWorkspaceStore();
    store.setProject("proj-worldmap");
    // 等重渲染：初始渲染时 projectId 尚为空，input 处于禁用态
    await nextTick();
    return { w, store };
  }

  it("展示上下文 chip（AI 研发负责人 · 当前页面上下文）", async () => {
    const { w } = await mounted();
    expect(w.text()).toContain("AI 研发负责人");
    expect(w.text()).toContain("当前页面上下文");
  });

  it("无项目上下文时输入禁用并有提示", () => {
    const w = mountShell(AICommandBar);
    const input = w.find("[data-test='command-input']");
    expect(input.attributes("disabled")).toBeDefined();
    expect(w.text()).toContain("请先选择项目");
  });

  it("发送 → user 气泡 + submitting（阶段文案 + 可取消）", async () => {
    const { w, store } = await mounted();
    await w
      .find("[data-test='command-input']")
      .setValue("分析当前迭代的主要风险");
    await w.find("[data-test='command-send']").trigger("click");
    expect(store.commandMessages.some(m => m.role === "user")).toBe(true);
    expect(w.find("[data-test='command-phase']").exists()).toBe(true);
    expect(w.find("[data-test='command-cancel']").exists()).toBe(true);
    // 生命周期硬规则：无回执不得出现完成态
    expect(store.commandMessages.some(m => m.role === "assistant")).toBe(false);
  });

  it("submitting 期间重复发送被忽略", async () => {
    const { w, store } = await mounted();
    const input = w.find("[data-test='command-input']");
    await input.setValue("第一条");
    await w.find("[data-test='command-send']").trigger("click");
    await input.setValue("第二条");
    await w.find("[data-test='command-send']").trigger("click");
    const userMessages = store.commandMessages.filter(m => m.role === "user");
    expect(userMessages).toHaveLength(1);
  });

  it("800ms 后 mock 回执 → assistant 气泡，回到 idle", async () => {
    const { w, store } = await mounted();
    await w
      .find("[data-test='command-input']")
      .setValue("分析当前迭代的主要风险");
    await w.find("[data-test='command-send']").trigger("click");
    vi.advanceTimersByTime(800);
    await nextTick();
    expect(store.commandMessages.some(m => m.role === "assistant")).toBe(true);
    expect(w.find("[data-test='command-phase']").exists()).toBe(false);
  });

  it("取消 → 无 assistant 气泡且回 idle", async () => {
    const { w, store } = await mounted();
    await w
      .find("[data-test='command-input']")
      .setValue("分析当前迭代的主要风险");
    await w.find("[data-test='command-send']").trigger("click");
    await w.find("[data-test='command-cancel']").trigger("click");
    vi.advanceTimersByTime(2000);
    await nextTick();
    expect(store.commandMessages.some(m => m.role === "assistant")).toBe(false);
    expect(w.find("[data-test='command-phase']").exists()).toBe(false);
  });

  it("Esc 清空本地消息", async () => {
    const { w, store } = await mounted();
    await w
      .find("[data-test='command-input']")
      .setValue("分析当前迭代的主要风险");
    await w.find("[data-test='command-send']").trigger("click");
    vi.advanceTimersByTime(800);
    await nextTick();
    expect(store.commandMessages.length).toBeGreaterThan(0);
    await w.find("[data-test='command-input']").trigger("keydown.esc");
    expect(store.commandMessages).toHaveLength(0);
  });

  it("「/ 引用」「@ Agent」为禁用占位", async () => {
    const { w } = await mounted();
    expect(
      w.find("[data-test='command-reference']").attributes("disabled")
    ).toBeDefined();
    expect(
      w.find("[data-test='command-at-agent']").attributes("disabled")
    ).toBeDefined();
  });

  it("Ctrl+/ 调用输入框 focus（聚焦接线）", async () => {
    const { w } = await mounted();
    const input = w.find("[data-test='command-input']");
    const focusSpy = vi.spyOn(input.element, "focus");
    window.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "/",
        ctrlKey: true,
        bubbles: true
      })
    );
    expect(focusSpy).toHaveBeenCalled();
  });
});
