// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";

import CommandBar from "./CommandBar.vue";
import { useShellStore } from "@/stores/shell";
import { mountShell } from "./test-utils";

/**
 * AI 指令栏两态（Application Shell §2E：收起 72px / 展开 120px）。
 */
describe("CommandBar", () => {
  it("收起态只显示输入行，不显示建议与作用域", async () => {
    const { wrapper } = mountShell(CommandBar);
    await nextTick();

    expect(wrapper.find(".shell-commandbar__scope").exists()).toBe(false);
    expect(wrapper.find(".shell-commandbar__hints").exists()).toBe(false);
    expect(wrapper.find(".shell-commandbar__input").exists()).toBe(true);
  });

  it("展开态显示作用域行、人工确认提示与建议芯片", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setCommandbarExpanded(true);
    await nextTick();

    expect(wrapper.find(".shell-commandbar__scope").exists()).toBe(true);
    expect(wrapper.find(".shell-commandbar__confirm").text()).toBe(
      "任何操作前请先确认"
    );
    expect(wrapper.findAll(".shell-commandbar__suggestion").length).toBe(2);
  });

  it("人工确认提示与 AI 建议使用不同视觉（不同颜色变量）", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setCommandbarExpanded(true);
    await nextTick();

    const confirm = wrapper.find(".shell-commandbar__confirm").element;
    const suggestion = wrapper.find(
      ".shell-commandbar__suggestions-label"
    ).element;
    expect(confirm.className).not.toBe(suggestion.className);
  });

  it("输入为空时发送按钮禁用，输入后启用", async () => {
    const { wrapper } = mountShell(CommandBar);
    await nextTick();

    const send = wrapper.find(".shell-commandbar__send");
    expect(send.attributes("disabled")).toBeDefined();

    await wrapper.find(".shell-commandbar__field").setValue("分析风险");
    await nextTick();

    expect(
      wrapper.find(".shell-commandbar__send").attributes("disabled")
    ).toBeUndefined();
  });

  it("收起态的占位符带当前上下文", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Atlas Platform" },
      { id: "p2", name: "Research Sandbox" }
    ]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "main",
      role: "main",
      status: "verified"
    };
    await nextTick();

    expect(
      wrapper.find(".shell-commandbar__field").attributes("placeholder")
    ).toBe("向 Atlas Platform / Repository / main 提问…");
  });

  it("切换中不显示旧项目名（无 stale project data）", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Atlas Platform" },
      { id: "p2", name: "Research Sandbox" }
    ]);
    shell.setCurrentProject("p1");
    await nextTick();

    // 挂起解析过程 → 停留在切换中
    void shell.switchProject("p2", () => new Promise<void>(() => {}));
    await nextTick();

    const placeholder = wrapper
      .find(".shell-commandbar__field")
      .attributes("placeholder")!;
    expect(placeholder).not.toContain("Atlas Platform");
    expect(placeholder).not.toContain("Research Sandbox");
    expect(placeholder).toContain("正在切换项目…");
  });
  it("发送键提交：入队待发提问、清空输入并进入会话页", async () => {
    const { wrapper, router } = mountShell(CommandBar);
    const shell = useShellStore();
    await router.push("/projects");
    await router.isReady();

    await wrapper.find(".shell-commandbar__field").setValue("  分析风险  ");
    await wrapper.find(".shell-commandbar__send").trigger("click");
    // /project/assistant 是懒加载路由：导航等动态 import 完成，需轮询等待
    await vi.waitFor(() =>
      expect(router.currentRoute.value.path).toBe("/project/assistant")
    );

    expect(shell.pendingAssistantPrompt?.text).toBe("分析风险");
    expect(
      (wrapper.find(".shell-commandbar__field").element as HTMLInputElement)
        .value
    ).toBe("");
  });

  it("Enter 提交；Shift+Enter 与输入法组字不提交", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    const field = wrapper.find(".shell-commandbar__field");

    await field.setValue("梳理需求");
    await field.trigger("keydown", { key: "Enter", shiftKey: true });
    await field.trigger("keydown", { key: "Enter", isComposing: true });
    expect(shell.pendingAssistantPrompt).toBeNull();

    await field.trigger("keydown", { key: "Enter" });
    expect(shell.pendingAssistantPrompt?.text).toBe("梳理需求");
  });

  it("建议芯片只填入输入框，不自动发送", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setCommandbarExpanded(true);
    await nextTick();

    const chip = wrapper.findAll(".shell-commandbar__suggestion")[0];
    await chip.trigger("click");

    expect(
      (wrapper.find(".shell-commandbar__field").element as HTMLInputElement)
        .value
    ).toBe(chip.text());
    expect(shell.pendingAssistantPrompt).toBeNull();
  });

  // ───────────────────────────────── 单一真源与快照交接（整合计划 Batch 1）

  it("作用域串与右上下文栏同源（均取 assistantContext）", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "main",
      role: "main",
      status: "verified"
    };
    await nextTick();

    expect(
      wrapper.find(".shell-commandbar__field").attributes("placeholder")
    ).toContain(`${shell.assistantContext.projectName} / Repository / main`);
  });

  it("提交时把上下文快照带入交接槽（不只传文本）", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "main",
      role: "main",
      status: "verified"
    };
    await nextTick();

    await wrapper.find(".shell-commandbar__field").setValue("分析风险");
    await wrapper.find(".shell-commandbar__send").trigger("click");
    await nextTick();

    expect(shell.pendingAssistantPrompt?.context).toEqual({
      projectId: "p1",
      projectName: "Atlas Platform",
      projectDescription: null,
      repositoryName: "atlas-platform",
      defaultBranch: "main",
      repositoryRole: "main"
    });
  });

  it("提交后切换项目，交接快照仍是提交时刻的上下文", async () => {
    const { wrapper } = mountShell(CommandBar);
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Atlas Platform" },
      { id: "p2", name: "Research Sandbox" }
    ]);
    shell.setCurrentProject("p1");
    await nextTick();

    await wrapper.find(".shell-commandbar__field").setValue("分析风险");
    await wrapper.find(".shell-commandbar__send").trigger("click");
    await nextTick();

    await shell.switchProject("p2");

    expect(shell.pendingAssistantPrompt?.context.projectId).toBe("p1");
    expect(shell.pendingAssistantPrompt?.context.projectName).toBe(
      "Atlas Platform"
    );
  });
});
