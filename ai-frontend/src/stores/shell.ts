import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { listRepositories } from "@/api/repositories";

/**
 * 应用壳状态（AI R&D Workspace Application Shell）。
 *
 * 壳自身的开关：
 *   - sidebarCollapsed：左导航折叠（展开 188px / 折叠 60px）
 *   - aiPanelOpen：AI 面板（AI Chat 446px + 右上下文 288px，**一体开关**——
 *     2026-11-12 规则：助手列与上下文栏是一个整体，顶栏「助手」按钮与
 *     AI 列头部的关闭按钮都作用于整个面板，默认全开）
 *   - commandbarExpanded：AI 指令栏（收起 72px / 展开 120px）
 *   - pendingAssistantPrompt：指令栏提交、待会话页（/project/assistant）消费的提问
 *   - focusMode：专注模式（DR-C8）——临时收起 AI Chat 与 Context Panel，
 *     主区独占工作区；退出逐项还原进入前的开合状态
 *
 * 项目上下文（Figma「04 — Global Navigation」Project Context Header /
 * 「07 — Project Switching」）：
 *   - currentProjectId：当前项目（空 = 未选择项目）
 *   - switchingProjectId：正在切换到的项目（非空 = 切换中）
 *   切换期间 **不暴露旧项目数据**（Figma：No stale project data is displayed），
 *   故 currentProject 在切换期间为 null，消费方显示切换态。
 *
 * 折叠态落在 CSS 属性开关上（壳根元素 data-sidebar / data-context /
 * data-focus），由 `.workspace-shell` 的 grid 模板消费——见 src/styles/index.css。
 */

/** 折叠态持久化键（与主题/语言键同族：aipm.*） */
export const SIDEBAR_STORAGE_KEY = "aipm.sidebar-collapsed";

/** 可选项目（切换菜单条目） */
export interface ProjectOption {
  id: string;
  name: string;
  /** 项目说明（右上下文栏 / 提示副文案用） */
  description?: string;
}

/** 当前项目的仓库上下文（右栏 / 指令栏作用域用；不可用时为 null） */
export interface RepositoryContext {
  name: string;
  defaultBranch: string;
  role: string;
  status: string;
}

/**
 * 助手上下文快照（提交时刻定格）。
 *
 * **单一真源的快照投影**：指令栏提交时把当时的上下文定格下来，交接给
 * 会话页——导航、切换项目都不改变这份快照，避免「问的是 A 项目、记到
 * B 项目」。字段与 `AssistantContext` 同构，便于直接回填。
 */
export interface AssistantContextSnapshot {
  /** 项目 id；切换中或未选项目时为 null */
  projectId: string | null;
  /** 项目名；切换中为 null（不回落到旧项目） */
  projectName: string | null;
  /** 项目说明 */
  projectDescription: string | null;
  /** 仓库名 */
  repositoryName: string | null;
  /** 默认分支 */
  defaultBranch: string | null;
  /** 本人角色 */
  repositoryRole: string | null;
}

/** 助手上下文（活态）——指令栏 / 右上下文栏 / 会话页共用同一对象 */
export type AssistantContext = AssistantContextSnapshot;

function initialSidebarCollapsed(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_STORAGE_KEY) === "1";
  } catch {
    // localStorage 不可用（隐私模式等）时用默认展开态
    return false;
  }
}

function persistSidebar(collapsed: boolean) {
  try {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, collapsed ? "1" : "0");
  } catch {
    // 持久化失败不影响本次切换
  }
}

export const useShellStore = defineStore("app-shell", () => {
  const sidebarCollapsed = ref(initialSidebarCollapsed());
  /** AI 面板一体开关：AI Chat 列 + 右上下文栏（默认全开）。 */
  const aiPanelOpen = ref(true);
  const commandbarExpanded = ref(false);
  /**
   * 专注模式（DR-C8）：临时收起 AI Chat 与 Context Panel，主区独占工作区。
   *
   * **只改布局，不改业务语义**：进入/退出都不触碰 currentProjectId、
   * assistantContext、pendingAssistantPrompt 等上下文与交接状态。
   *
   * 进入前记录 AI 面板开合态（aiPanelOpen），退出时逐项还原——专注模式
   * 不吞掉用户原本的面板开关选择。
   */
  const focusMode = ref(false);
  const aiPanelOpenBeforeFocus = ref<boolean | null>(null);
  /**
   * 指令栏 → 会话页的单槽交接：指令栏只入队并导航，真正发送由会话页走
   * 其既有发送链（自动建会话 / 失败重发）；消费即清空，避免重复发送。
   *
   * **携带上下文快照**（整合计划 Batch 1）：提交时刻定格，导航/切换项目
   * 都不改变它——会话页据此把问题落到「问的时候的那个项目」。
   */
  const pendingAssistantPrompt = ref<{
    text: string;
    context: AssistantContextSnapshot;
  } | null>(null);

  /** 可选项目清单（骨架阶段由调用方注入，接真实数据源时替换） */
  const projects = ref<ProjectOption[]>([]);
  /** 当前项目 id（空 = 未选择项目） */
  const currentProjectId = ref<string | null>(null);
  /** 正在切换到的项目 id（非空 = 切换中） */
  const switchingProjectId = ref<string | null>(null);
  /** 当前项目的仓库上下文（无仓库 / 接口不可用时 null） */
  const currentRepository = ref<RepositoryContext | null>(null);

  /** 壳根元素的 data-sidebar 取值 */
  const sidebarState = computed(() =>
    sidebarCollapsed.value ? "collapsed" : "expanded"
  );

  /** 壳根元素的 data-context 取值（AI 面板一体开合 → grid 列模板切换） */
  const contextState = computed(() => (aiPanelOpen.value ? "open" : "closed"));

  /** 壳根元素的 data-focus 取值（专注模式） */
  const focusState = computed(() => (focusMode.value ? "on" : "off"));

  /** 切换中（期间不暴露旧项目数据） */
  const switching = computed(() => switchingProjectId.value !== null);

  /**
   * 当前项目。
   * 切换期间为 null —— Figma「07」要求切换中不得显示旧项目数据，
   * 消费方据此显示切换态而非旧内容。
   */
  const currentProject = computed<ProjectOption | null>(() => {
    if (switching.value) return null;
    return projects.value.find(p => p.id === currentProjectId.value) ?? null;
  });

  /** 切换中的目标项目（用于「正在打开 X…」提示） */
  const switchingProject = computed<ProjectOption | null>(
    () => projects.value.find(p => p.id === switchingProjectId.value) ?? null
  );

  /**
   * 助手上下文（活态单一真源）。
   *
   * 指令栏作用域串、右上下文栏「助手可用的上下文」卡、会话页三处**共用
   * 这一份**，不再各自派生（F-1/F-2/F-8 的派生分裂根因）。
   *
   * 切换期间项目与仓库字段全为 null —— 不暴露旧项目数据
   * （Figma「07」No stale project data is displayed）。仓库随项目切换
   * 异步重载，故切换窗口内旧分支同样不得外泄。
   */
  const assistantContext = computed<AssistantContext>(() => {
    if (switching.value) {
      return {
        projectId: null,
        projectName: null,
        projectDescription: null,
        repositoryName: null,
        defaultBranch: null,
        repositoryRole: null
      };
    }
    const project = currentProject.value;
    const repository = currentRepository.value;
    return {
      projectId: project?.id ?? null,
      projectName: project?.name ?? null,
      projectDescription: project?.description ?? null,
      repositoryName: repository?.name ?? null,
      defaultBranch: repository?.defaultBranch ?? null,
      repositoryRole: repository?.role ?? null
    };
  });

  function toggleSidebar() {
    setSidebarCollapsed(!sidebarCollapsed.value);
  }

  function setSidebarCollapsed(collapsed: boolean) {
    if (sidebarCollapsed.value === collapsed) return;
    sidebarCollapsed.value = collapsed;
    persistSidebar(collapsed);
  }

  /** AI 面板一体开关（AI Chat 列 + 右上下文栏同时开合）。 */
  function toggleAiPanel() {
    aiPanelOpen.value = !aiPanelOpen.value;
  }

  function setAiPanelOpen(open: boolean) {
    aiPanelOpen.value = open;
  }

  function toggleCommandbar() {
    commandbarExpanded.value = !commandbarExpanded.value;
  }

  function setCommandbarExpanded(expanded: boolean) {
    commandbarExpanded.value = expanded;
  }

  /**
   * 进入专注模式：收起 AI Chat 与 Context Panel，主区独占。
   *
   * 进入前记录右栏开合态（供退出时还原）；**不改动**任何项目 / 会话 /
   * 上下文状态，故当前编辑内容与对话状态天然保留。
   */
  function enterFocusMode() {
    if (focusMode.value) return;
    aiPanelOpenBeforeFocus.value = aiPanelOpen.value;
    focusMode.value = true;
  }

  /**
   * 退出专注模式：恢复布局，并还原进入前的 AI 面板开合态。
   * 专注模式期间用户若手动改了面板开关，以用户最后一次选择为准。
   */
  function exitFocusMode() {
    if (!focusMode.value) return;
    focusMode.value = false;
    if (aiPanelOpenBeforeFocus.value !== null) {
      aiPanelOpen.value = aiPanelOpenBeforeFocus.value;
      aiPanelOpenBeforeFocus.value = null;
    }
  }

  function toggleFocusMode() {
    if (focusMode.value) {
      exitFocusMode();
    } else {
      enterFocusMode();
    }
  }

  /** 指令栏提交提问（空白忽略）；会话页经 takeAssistantPrompt 消费。
   *  槽用新对象承载（而非裸字符串）：相同文案连续提交也能再次触发
   *  会话页对 pendingAssistantPrompt 的 watch。
   *  **上下文在提交时刻定格**（快照），导航/切换项目不改变它。 */
  function queueAssistantPrompt(text: string) {
    const value = text.trim();
    if (value) {
      pendingAssistantPrompt.value = {
        text: value,
        context: { ...assistantContext.value }
      };
    }
  }

  /** 取出并清空待发提问（无则 null） */
  function takeAssistantPrompt(): {
    text: string;
    context: AssistantContextSnapshot;
  } | null {
    const value = pendingAssistantPrompt.value;
    pendingAssistantPrompt.value = null;
    return value;
  }

  /** 设置可选项目清单（骨架阶段由演示数据注入） */
  function setProjects(list: ProjectOption[]) {
    projects.value = list;
  }

  /** 直接选定当前项目（无切换过程，如初始化） */
  function setCurrentProject(id: string | null) {
    currentProjectId.value = id;
  }

  /** 载入当前项目的仓库上下文（main 仓库优先；失败即空态，不伪造仓库） */
  async function loadRepositoryContext(projectId: string | null) {
    currentRepository.value = null;
    if (!projectId) return;
    try {
      const { items } = await listRepositories(projectId);
      const main = items.find(repo => repo.role === "main") ?? items[0];
      if (!main) return;
      currentRepository.value = {
        name: main.name,
        defaultBranch: main.default_branch ?? "",
        role: main.role,
        status: main.status
      };
    } catch {
      currentRepository.value = null;
    }
  }

  /**
   * 待确认的切项目目标（基线 G12 · DR-C6）。
   *
   * 切项目**不得静默转移会话与上下文**：当前有活跃会话时先确认——
   * 确认后在目标项目另起会话（历史会话保留原项目归属），取消则维持现状。
   * 无活跃会话时直接切换，不打扰。
   */
  const pendingProjectSwitch = ref<string | null>(null);

  /**
   * 切换项目。
   *
   * 两阶段：先进入 switching 态（导航与页面显示切换态，不显示旧项目数据），
   * 待新上下文就绪后再落地 currentProjectId。
   *
   * @param id 目标项目 id
   * @param resolve 可选的「解析新上下文」过程（真实实现里是拉取项目详情/权限）；
   *                不传则下一微任务即完成。解析失败时保持切换态并抛出。
   */
  async function switchProject(
    id: string,
    resolve?: () => Promise<unknown>
  ): Promise<void> {
    if (id === currentProjectId.value) return;
    switchingProjectId.value = id;
    try {
      if (resolve) await resolve();
      currentProjectId.value = id;
    } finally {
      switchingProjectId.value = null;
    }
  }

  /**
   * 请求切换项目（切项目入口统一走这里）。
   *
   * @param id 目标项目 id
   * @param hasActiveConversation 当前是否有活跃会话（有则先确认）
   */
  function requestProjectSwitch(id: string, hasActiveConversation: boolean) {
    if (id === currentProjectId.value) return;
    if (hasActiveConversation) {
      pendingProjectSwitch.value = id;
      return;
    }
    void switchProject(id);
  }

  /** 确认切换：落地目标项目（会话在目标项目另起，历史归属不变）。 */
  async function confirmProjectSwitch() {
    const id = pendingProjectSwitch.value;
    pendingProjectSwitch.value = null;
    if (!id) return;
    await switchProject(id);
  }

  /** 取消切换：维持当前项目与上下文。 */
  function cancelProjectSwitch() {
    pendingProjectSwitch.value = null;
  }

  return {
    sidebarCollapsed,
    aiPanelOpen,
    commandbarExpanded,
    focusMode,
    pendingProjectSwitch,
    pendingAssistantPrompt,
    projects,
    currentProjectId,
    switchingProjectId,
    currentRepository,
    sidebarState,
    contextState,
    focusState,
    switching,
    currentProject,
    switchingProject,
    assistantContext,
    toggleSidebar,
    setSidebarCollapsed,
    toggleAiPanel,
    setAiPanelOpen,
    toggleCommandbar,
    setCommandbarExpanded,
    requestProjectSwitch,
    confirmProjectSwitch,
    cancelProjectSwitch,
    enterFocusMode,
    exitFocusMode,
    toggleFocusMode,
    queueAssistantPrompt,
    takeAssistantPrompt,
    setProjects,
    setCurrentProject,
    loadRepositoryContext,
    switchProject
  };
});
