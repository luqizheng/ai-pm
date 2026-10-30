// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { attentionItems } from "./fixtures";
import { useWorkspaceStore } from "./store";

describe("useWorkspaceStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("setProject 记录项目上下文", () => {
    const s = useWorkspaceStore();
    expect(s.projectId).toBe("");
    s.setProject("proj-worldmap");
    expect(s.projectId).toBe("proj-worldmap");
  });

  it("selectAttention 记录选中项，selectedAttention 返回对应数据", () => {
    const s = useWorkspaceStore();
    s.selectAttention(attentionItems[1].id);
    expect(s.selectedAttentionId).toBe(attentionItems[1].id);
    expect(s.selectedAttention?.title).toBe(attentionItems[1].title);
  });

  it("selectAttention(null) 清空选中", () => {
    const s = useWorkspaceStore();
    s.selectAttention(attentionItems[0].id);
    s.selectAttention(null);
    expect(s.selectedAttentionId).toBeNull();
    expect(s.selectedAttention).toBeUndefined();
  });

  it("栏开关：toggleSidebar / toggleContext 往返", () => {
    const s = useWorkspaceStore();
    expect(s.sidebarCollapsed).toBe(false);
    expect(s.contextOpen).toBe(true);
    s.toggleSidebar();
    s.toggleContext();
    expect(s.sidebarCollapsed).toBe(true);
    expect(s.contextOpen).toBe(false);
  });

  it("指令栏消息：追加带唯一 id，可清空", () => {
    const s = useWorkspaceStore();
    s.pushCommandMessage("user", "分析当前迭代的主要风险");
    s.pushCommandMessage("assistant", "迭代整体正常，存在一个架构决策。");
    expect(s.commandMessages).toHaveLength(2);
    expect(s.commandMessages[0].role).toBe("user");
    expect(s.commandMessages[1].role).toBe("assistant");
    expect(s.commandMessages[0].id).not.toBe(s.commandMessages[1].id);
    s.clearCommandMessages();
    expect(s.commandMessages).toHaveLength(0);
  });
});
