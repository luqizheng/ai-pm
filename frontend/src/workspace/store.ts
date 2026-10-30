import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { attentionItems, type AttentionItem } from "./fixtures";

export interface CommandMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
}

let messageSeq = 0;

/**
 * 工作区现场状态（设计系统 V1.0 §六 state/workspace-store 的 Vue 落地）：
 * 项目上下文、选中待办、栏开关、指令栏本地消息。
 */
export const useWorkspaceStore = defineStore("workspace", () => {
  const projectId = ref("");
  const selectedAttentionId = ref<string | null>(null);
  const sidebarCollapsed = ref(false);
  const contextOpen = ref(true);
  const commandMessages = ref<CommandMessage[]>([]);

  const selectedAttention = computed<AttentionItem | undefined>(() =>
    attentionItems.find(item => item.id === selectedAttentionId.value)
  );

  function setProject(id: string) {
    projectId.value = id;
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

  function pushCommandMessage(role: CommandMessage["role"], text: string) {
    messageSeq += 1;
    commandMessages.value.push({ id: `msg-${messageSeq}`, role, text });
  }

  function clearCommandMessages() {
    commandMessages.value = [];
  }

  return {
    projectId,
    selectedAttentionId,
    selectedAttention,
    sidebarCollapsed,
    contextOpen,
    commandMessages,
    setProject,
    selectAttention,
    toggleSidebar,
    toggleContext,
    pushCommandMessage,
    clearCommandMessages
  };
});
