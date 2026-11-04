import { computed, ref } from "vue";
import { defineStore } from "pinia";

/**
 * 应用壳状态（AI R&D Workspace Application Shell）。
 *
 * 只管壳自身的三个开关与当前上下文，不含任何业务状态：
 *   - sidebarCollapsed：左导航折叠（展开 232px / 折叠 60px）
 *   - contextOpen：右上下文栏（320px，可关）
 *   - commandbarExpanded：AI 指令栏（收起 72px / 展开 120px）
 *
 * 折叠态落在 CSS 属性开关上（壳根元素 data-sidebar / data-context），
 * 由 `.workspace-shell` 的 grid 模板消费——见 src/styles/index.css。
 */

/** 折叠态持久化键（与主题/语言键同族：aipm.*） */
export const SIDEBAR_STORAGE_KEY = "aipm.sidebar-collapsed";

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

  /** 壳根元素的 data-sidebar 取值 */
  const sidebarState = computed(() =>
    sidebarCollapsed.value ? "collapsed" : "expanded"
  );

  /** 壳根元素的 data-context 取值 */
  const contextState = computed(() => (contextOpen.value ? "open" : "closed"));

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

  return {
    sidebarCollapsed,
    contextOpen,
    commandbarExpanded,
    sidebarState,
    contextState,
    toggleSidebar,
    setSidebarCollapsed,
    toggleContext,
    setContextOpen,
    toggleCommandbar,
    setCommandbarExpanded
  };
});
