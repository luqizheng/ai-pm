import { computed, ref } from "vue";
import { defineStore } from "pinia";

/**
 * 应用壳状态（AI R&D Workspace Application Shell）。
 *
 * 壳自身的开关：
 *   - sidebarCollapsed：左导航折叠（展开 232px / 折叠 60px）
 *   - contextOpen：右上下文栏（320px，可关）
 *   - commandbarExpanded：AI 指令栏（收起 72px / 展开 120px）
 *
 * 项目上下文（Figma「04 — Global Navigation」Project Context Header /
 * 「07 — Project Switching」）：
 *   - currentProjectId：当前项目（空 = 未选择项目）
 *   - switchingProjectId：正在切换到的项目（非空 = 切换中）
 *   切换期间 **不暴露旧项目数据**（Figma：No stale project data is displayed），
 *   故 currentProject 在切换期间为 null，消费方显示切换态。
 *
 * 折叠态落在 CSS 属性开关上（壳根元素 data-sidebar / data-context），
 * 由 `.workspace-shell` 的 grid 模板消费——见 src/styles/index.css。
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
  const contextOpen = ref(true);
  const commandbarExpanded = ref(false);

  /** 可选项目清单（骨架阶段由调用方注入，接真实数据源时替换） */
  const projects = ref<ProjectOption[]>([]);
  /** 当前项目 id（空 = 未选择项目） */
  const currentProjectId = ref<string | null>(null);
  /** 正在切换到的项目 id（非空 = 切换中） */
  const switchingProjectId = ref<string | null>(null);

  /** 壳根元素的 data-sidebar 取值 */
  const sidebarState = computed(() =>
    sidebarCollapsed.value ? "collapsed" : "expanded"
  );

  /** 壳根元素的 data-context 取值 */
  const contextState = computed(() => (contextOpen.value ? "open" : "closed"));

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

  function toggleSidebar() {
    setSidebarCollapsed(!sidebarCollapsed.value);
  }

  function setSidebarCollapsed(collapsed: boolean) {
    if (sidebarCollapsed.value === collapsed) return;
    sidebarCollapsed.value = collapsed;
    persistSidebar(collapsed);
  }

  function toggleContext() {
    contextOpen.value = !contextOpen.value;
  }

  function setContextOpen(open: boolean) {
    contextOpen.value = open;
  }

  function toggleCommandbar() {
    commandbarExpanded.value = !commandbarExpanded.value;
  }

  function setCommandbarExpanded(expanded: boolean) {
    commandbarExpanded.value = expanded;
  }

  /** 设置可选项目清单（骨架阶段由演示数据注入） */
  function setProjects(list: ProjectOption[]) {
    projects.value = list;
  }

  /** 直接选定当前项目（无切换过程，如初始化） */
  function setCurrentProject(id: string | null) {
    currentProjectId.value = id;
  }

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

  return {
    sidebarCollapsed,
    contextOpen,
    commandbarExpanded,
    projects,
    currentProjectId,
    switchingProjectId,
    sidebarState,
    contextState,
    switching,
    currentProject,
    switchingProject,
    toggleSidebar,
    setSidebarCollapsed,
    toggleContext,
    setContextOpen,
    toggleCommandbar,
    setCommandbarExpanded,
    setProjects,
    setCurrentProject,
    switchProject
  };
});
