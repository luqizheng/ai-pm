// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { useAssistantStore } from "@/stores/assistant";
import { useShellStore } from "@/stores/shell";

import ProjectSwitchGuardDialog from "./ProjectSwitchGuardDialog.vue";

/**
 * 切项目确认（G12 · DR-C6）：切项目不得静默转移会话与上下文。
 *
 * 确认对话框 Teleport 到 body —— 用组件事件驱动（不查 DOM）。
 */
describe("ProjectSwitchGuardDialog", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  function seedProjects() {
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Atlas Platform" },
      { id: "p2", name: "Smark Platform" }
    ]);
    shell.setCurrentProject("p1");
    return shell;
  }

  it("无待切换目标时不渲染对话框", async () => {
    const { wrapper } = mountShell(ProjectSwitchGuardDialog);
    await nextTick();

    expect(wrapper.findComponent(ConfirmationDialog).props("open")).toBe(false);
  });

  it("有待切换目标时显示确认文案与两个动作", async () => {
    const { wrapper } = mountShell(ProjectSwitchGuardDialog);
    const shell = seedProjects();
    shell.requestProjectSwitch("p2", true);
    await nextTick();

    const dialog = wrapper.findComponent(ConfirmationDialog);
    expect(dialog.props("open")).toBe(true);
    const text = String(dialog.props("description"));
    expect(text).toContain("Smark Platform");
    // 明说「不跟随」——不得让用户以为会话会带过去
    expect(text).toContain("不会跟着转移");
    expect(dialog.props("confirmLabel")).toBe("确认切换");
    expect(dialog.props("cancelLabel")).toBe("取消");
  });

  it("确认：落到目标项目并清空待切换态", async () => {
    const { wrapper } = mountShell(ProjectSwitchGuardDialog);
    const shell = seedProjects();
    shell.requestProjectSwitch("p2", true);
    await nextTick();

    wrapper.findComponent(ConfirmationDialog).vm.$emit("confirm");
    await nextTick();
    await nextTick();

    expect(shell.currentProjectId).toBe("p2");
    expect(shell.pendingProjectSwitch).toBe(null);
  });

  it("取消：维持当前项目与上下文", async () => {
    const { wrapper } = mountShell(ProjectSwitchGuardDialog);
    const shell = seedProjects();
    shell.requestProjectSwitch("p2", true);
    await nextTick();

    wrapper.findComponent(ConfirmationDialog).vm.$emit("update:open", false);
    await nextTick();

    expect(shell.currentProjectId).toBe("p1");
    expect(shell.pendingProjectSwitch).toBe(null);
  });

  it("无活跃会话时直接切换，不打扰", async () => {
    const { wrapper } = mountShell(ProjectSwitchGuardDialog);
    const shell = seedProjects();
    const assistant = useAssistantStore();
    assistant.setActiveConversationId(null);

    shell.requestProjectSwitch("p2", false);
    await nextTick();
    await nextTick();

    expect(shell.pendingProjectSwitch).toBe(null);
    expect(shell.currentProjectId).toBe("p2");
    expect(wrapper.findComponent(ConfirmationDialog).props("open")).toBe(false);
  });
});
