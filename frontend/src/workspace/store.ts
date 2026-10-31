import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { attentionItems, type AttentionItem } from "./fixtures";
import type { NavMode } from "./nav-config";

export interface CommandMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
}

/** /focus 的主对象（WorkspaceSpec.subject 的本地形态）。 */
export interface WorkspaceSubject {
  type: string;
  id: string;
}

/** 可恢复的页面现场：query + 滚动位置（无损往返 §1.4）。 */
export interface ViewState {
  query: Record<string, string>;
  scrollTop: number;
}

let messageSeq = 0;

/**
 * 工作区现场状态（设计系统 V1.0 §六 state/workspace-store 的 Vue 落地）：
 * 项目上下文、选中待办、栏开关、指令栏本地消息。
 */
export const useWorkspaceStore = defineStore("workspace", () => {
  const projectId = ref("");
  const subject = ref<WorkspaceSubject | null>(null);
  const viewStates = ref<Record<string, ViewState>>({});
  const selectedAttentionId = ref<string | null>(null);
  const sidebarCollapsed = ref(false);
  const contextOpen = ref(true);
  const commandMessages = ref<CommandMessage[]>([]);
  const navMode = ref<NavMode>("classic");

  const selectedAttention = computed<AttentionItem | undefined>(() =>
    attentionItems.find(item => item.id === selectedAttentionId.value)
  );

  function setProject(id: string) {
    projectId.value = id;
  }

  function setSubject(next: WorkspaceSubject | null) {
    subject.value = next;
  }

  /** 进入 /focus 前记录来源页现场，按 route name 分桶。 */
  function captureViewState(
    route: { name?: string | symbol | null; query?: Record<string, unknown> },
    scrollTop = 0
  ) {
    const key = String(route.name ?? "");
    if (!key) return;
    const query: Record<string, string> = {};
    for (const [name, value] of Object.entries(route.query ?? {})) {
      if (typeof value === "string") query[name] = value;
    }
    viewStates.value[key] = { query, scrollTop };
  }

  function restoreViewState(routeName: string): ViewState | undefined {
    return viewStates.value[routeName];
  }

  function selectAttention(id: string | null) {
    selectedAttentionId.value = id;
  }

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value;
  }

  function toggleContext() {
    contextOpen.value = !contextOpen.value;
  }

  function setNavMode(mode: NavMode) {
    navMode.value = mode;
  }

  function pushCommandMessage(role: CommandMessage["role"], text: string) {
    messageSeq += 1;
    commandMessages.value.push({ id: `msg-${messageSeq}`, role, text });
  }

  function clearCommandMessages() {
    commandMessages.value = [];
  }

  return {
    projectId,
    subject,
    selectedAttentionId,
    selectedAttention,
    sidebarCollapsed,
    contextOpen,
    commandMessages,
    navMode,
    setProject,
    setSubject,
    captureViewState,
    restoreViewState,
    selectAttention,
    toggleSidebar,
    toggleContext,
    setNavMode,
    pushCommandMessage,
    clearCommandMessages
  };
});
