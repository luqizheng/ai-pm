// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";

import { SIDEBAR_STORAGE_KEY, useShellStore } from "./shell";

/**
 * 壳状态 store 契约。
 */
describe("shell store", () => {
  beforeEach(() => {
    localStorage.clear();
    setActivePinia(createPinia());
  });

  it("默认展开：左导航展开、AI 面板打开（助手+上下文一体）、指令栏收起", () => {
    const shell = useShellStore();

    expect(shell.sidebarCollapsed).toBe(false);
    expect(shell.sidebarState).toBe("expanded");
    expect(shell.aiPanelOpen).toBe(true);
    expect(shell.contextState).toBe("open");
    expect(shell.commandbarExpanded).toBe(false);
  });

  it("切换左导航折叠态并持久化", () => {
    const shell = useShellStore();

    shell.toggleSidebar();

    expect(shell.sidebarCollapsed).toBe(true);
    expect(shell.sidebarState).toBe("collapsed");
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe("1");
  });

  it("重复设置相同折叠态不重复写入", () => {
    const shell = useShellStore();

    shell.setSidebarCollapsed(false);
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBeNull();

    shell.setSidebarCollapsed(true);
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe("1");

    shell.setSidebarCollapsed(true);
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe("1");
  });

  it("从持久化值恢复折叠态", () => {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, "1");
    setActivePinia(createPinia());

    expect(useShellStore().sidebarCollapsed).toBe(true);
  });

  it("AI 面板总开关：全开 ⇄ 全关；上下文单独关闭后一键补开", () => {
    const shell = useShellStore();

    // 全开 → 全关
    shell.toggleAiPanel();
    expect(shell.aiPanelOpen).toBe(false);
    expect(shell.panelState).toBe("closed");
    expect(shell.contextState).toBe("closed");

    // 全关 → 全开（上下文一并补开，开启即全开）
    shell.toggleAiPanel();
    expect(shell.aiPanelOpen).toBe(true);
    expect(shell.contextOpen).toBe(true);
    expect(shell.panelState).toBe("open");
    expect(shell.contextState).toBe("open");

    // 上下文栏单独关闭（AI 列头部关闭按钮）：助手列保留
    shell.setContextOpen(false);
    expect(shell.contextOpen).toBe(false);
    expect(shell.aiPanelOpen).toBe(true);
    expect(shell.panelState).toBe("open");
    expect(shell.contextState).toBe("closed");

    // 顶栏「助手」一键补开 → 回归全开
    shell.toggleAiPanel();
    expect(shell.contextOpen).toBe(true);
    expect(shell.aiPanelOpen).toBe(true);

    shell.toggleCommandbar();
    expect(shell.commandbarExpanded).toBe(true);

    shell.setCommandbarExpanded(false);
    expect(shell.commandbarExpanded).toBe(false);
  });

  it("项目上下文：设置清单与当前项目", () => {
    const shell = useShellStore();

    shell.setProjects([
      { id: "p1", name: "Atlas Platform" },
      { id: "p2", name: "Research Sandbox" }
    ]);
    shell.setCurrentProject("p1");

    expect(shell.projects.length).toBe(2);
    expect(shell.currentProjectId).toBe("p1");
    expect(shell.currentProject?.name).toBe("Atlas Platform");
  });

  it("未选项目时 currentProject 为 null（Empty 态）", () => {
    const shell = useShellStore();

    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject(null);

    expect(shell.currentProject).toBeNull();
    expect(shell.switching).toBe(false);
  });

  it("切换项目：先进入 switching 态，完成后落地新项目", async () => {
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Atlas Platform" },
      { id: "p2", name: "Research Sandbox" }
    ]);
    shell.setCurrentProject("p1");

    let release: () => void = () => {};
    const pending = shell.switchProject(
      "p2",
      () =>
        new Promise<void>(resolve => {
          release = resolve;
        })
    );

    // 切换中：标记目标项目，且**不暴露旧项目数据**
    expect(shell.switching).toBe(true);
    expect(shell.switchingProjectId).toBe("p2");
    expect(shell.switchingProject?.name).toBe("Research Sandbox");
    expect(shell.currentProject).toBeNull();

    release();
    await pending;

    expect(shell.switching).toBe(false);
    expect(shell.currentProjectId).toBe("p2");
    expect(shell.currentProject?.name).toBe("Research Sandbox");
  });

  it("切换到当前项目不进入切换态", async () => {
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");

    await shell.switchProject("p1");

    expect(shell.switching).toBe(false);
    expect(shell.currentProjectId).toBe("p1");
  });

  // ───────────────────────────────────────────── 助手上下文单一真源

  it("assistantContext 由当前项目与仓库派生（指令栏/右栏/会话页共用）", () => {
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Atlas Platform", description: "Acme Research" }
    ]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "main",
      role: "main",
      status: "verified"
    };

    expect(shell.assistantContext).toEqual({
      projectId: "p1",
      projectName: "Atlas Platform",
      projectDescription: "Acme Research",
      repositoryName: "atlas-platform",
      defaultBranch: "main",
      repositoryRole: "main"
    });
  });

  it("切换中 assistantContext 不暴露旧项目数据", async () => {
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

    void shell.switchProject("p2", () => new Promise<void>(() => {}));

    expect(shell.assistantContext.projectId).toBeNull();
    expect(shell.assistantContext.projectName).toBeNull();
    expect(shell.assistantContext.defaultBranch).toBeNull();
  });

  it("入队提问时定格上下文快照", () => {
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "main",
      role: "main",
      status: "verified"
    };

    shell.queueAssistantPrompt("分析风险");

    expect(shell.pendingAssistantPrompt?.text).toBe("分析风险");
    expect(shell.pendingAssistantPrompt?.context).toEqual({
      projectId: "p1",
      projectName: "Atlas Platform",
      projectDescription: null,
      repositoryName: "atlas-platform",
      defaultBranch: "main",
      repositoryRole: "main"
    });
  });

  it("提交后切换项目，快照不漂移（仍是提交时刻的上下文）", async () => {
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Atlas Platform" },
      { id: "p2", name: "Research Sandbox" }
    ]);
    shell.setCurrentProject("p1");

    shell.queueAssistantPrompt("分析风险");
    await shell.switchProject("p2");

    expect(shell.pendingAssistantPrompt?.context.projectId).toBe("p1");
    expect(shell.pendingAssistantPrompt?.context.projectName).toBe(
      "Atlas Platform"
    );
  });

  it("取出待发提问即清空槽（消费即清空，不重复发送）", () => {
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    shell.queueAssistantPrompt("分析风险");

    const taken = shell.takeAssistantPrompt();

    expect(taken?.text).toBe("分析风险");
    expect(taken?.context.projectId).toBe("p1");
    expect(shell.pendingAssistantPrompt).toBeNull();
  });
});
