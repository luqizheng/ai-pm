import { computed, ref } from "vue";
import { defineStore } from "pinia";

/**
 * 助手会话派生视图（AI Collaboration V1.0 / DR-C8 第 ③ 步）。
 *
 * **写者唯一**：`useAssistantConversation`（会话机制 composable）在加载消息 /
 * 写回卡片后写入；**读者两处**：AI Chat 常驻列与右侧 Context Panel。
 *
 * 为什么要这一层：右栏与列是**兄弟组件**（都在壳里），若各自拉会话会形成
 * 两套状态（一致性风险 + 重复请求）。派生数据落 store 后，右栏展示的
 * 「AI 看到了什么 / 产出了什么 / 做了什么」与对话区**同源**。
 *
 * 诚实性：Sources 只展示**真实注入记账**（后端 `metadata.injection`），
 * 没有注入数据时如实显示未注入，**不冒充**注入面（G2 断点的正解）。
 */

/** 右栏四个一级 Tab（DR-C3）。 */
export type AssistantTab = "context" | "sources" | "results" | "actions";

/** History Drawer 的五类记录（DR-C3）。 */
export type AssistantHistoryCategory =
  | "conversation"
  | "ai_action"
  | "context_change"
  | "approval"
  | "execution";

/** 后端注入记账（`message.metadata.injection`）。 */
export interface AssistantInjection {
  /** 本次实际注入的分节标识 */
  sectionsIncluded: string[];
  /** 因预算被丢弃的分节标识 */
  dropped: string[];
  /** 注入字符总量 */
  charsTotal: number;
}

/** 一条 AI 结果（由时间线里的 assistant 消息 + 信封派生）。 */
export interface AssistantResultItem {
  id: string;
  /** 信封类型（8 类 RESPONSE_TYPES）或 "text"（无信封的纯文本回复） */
  responseType: string;
  title: string;
  status: string;
  createdAt: string;
}

/** 一条动作（AI 请求 / 用户授权 / 执行结果）。 */
export interface AssistantActionItem {
  id: string;
  /** 信封类型或动作标识 */
  kind: string;
  /** 发起方：ai = AI 请求，user = 人工确认 */
  initiator: "ai" | "user";
  status: string;
  createdAt: string;
}

/** 一条历史事件（Drawer 五类之一）。 */
export interface AssistantHistoryEvent {
  id: string;
  category: AssistantHistoryCategory;
  name: string;
  actor: string;
  createdAt: string;
  status?: string;
  detail?: string;
}

export const useAssistantStore = defineStore("assistant-view", () => {
  /** 右栏当前一级 Tab */
  const activeTab = ref<AssistantTab>("context");
  /** 真实注入记账（无则 null —— 未收到回复即未注入） */
  const injection = ref<AssistantInjection | null>(null);
  /** 当前会话的 AI 结果 */
  const results = ref<AssistantResultItem[]>([]);
  /** 当前会话的动作 */
  const actions = ref<AssistantActionItem[]>([]);
  /** 历史事件（五类合并，按时间倒序由消费方排序） */
  const history = ref<AssistantHistoryEvent[]>([]);
  /** History Drawer 是否打开 */
  const historyOpen = ref(false);
  /** History Drawer 当前筛选类别（"all" = 全部） */
  const historyCategory = ref<AssistantHistoryCategory | "all">("all");

  /** 按类别过滤后的历史事件（Drawer 用）。 */
  const filteredHistory = computed(() =>
    historyCategory.value === "all"
      ? history.value
      : history.value.filter(event => event.category === historyCategory.value)
  );

  function setActiveTab(tab: AssistantTab) {
    activeTab.value = tab;
  }

  function setInjection(value: AssistantInjection | null) {
    injection.value = value;
  }

  function setResults(items: AssistantResultItem[]) {
    results.value = items;
  }

  function setActions(items: AssistantActionItem[]) {
    actions.value = items;
  }

  function setHistory(events: AssistantHistoryEvent[]) {
    history.value = events;
  }

  /** 追加一条历史事件（去重：同 id 覆盖）。 */
  function pushHistory(event: AssistantHistoryEvent) {
    const index = history.value.findIndex(item => item.id === event.id);
    if (index >= 0) {
      history.value[index] = event;
      return;
    }
    history.value = [...history.value, event];
  }

  function openHistory(category: AssistantHistoryCategory | "all" = "all") {
    historyCategory.value = category;
    historyOpen.value = true;
  }

  function closeHistory() {
    historyOpen.value = false;
  }

  /** 切换项目/会话：清空派生视图（不串项目）。 */
  function reset() {
    injection.value = null;
    results.value = [];
    actions.value = [];
    history.value = [];
    historyCategory.value = "all";
  }

  return {
    activeTab,
    injection,
    results,
    actions,
    history,
    historyOpen,
    historyCategory,
    filteredHistory,
    setActiveTab,
    setInjection,
    setResults,
    setActions,
    setHistory,
    pushHistory,
    openHistory,
    closeHistory,
    reset
  };
});
