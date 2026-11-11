import { computed, onUnmounted, ref, type Ref } from "vue";

import {
  appendMessage,
  createConversation,
  listConversations,
  listMessages,
  submitInteraction,
  type Conversation,
  type ConversationContextAnchor,
  type ConversationMessage as ConversationMessageRecord,
  type SendMessageOutcome,
  type SubmitInteractionPayload
} from "@/api/conversations";
import {
  INTERACTION_STATUSES,
  PAYLOAD_REQUIRED_FIELDS,
  RESPONSE_TYPES,
  type AiEvidence,
  type AiIdentity,
  type AiInteractionEnvelope,
  type AiResponseType,
  type ConversationMessage,
  type InteractionStatus
} from "@/components/ui/ai-interaction/types";
import { useShellStore, type AssistantContextSnapshot } from "@/stores/shell";

/**
 * Project AI 会话机制（AI Collaboration V1.0 / DR-C8 第 ② 步的复用底座）。
 *
 * **单一实现，双处消费**：会话页（`/project/assistant`）与 AI Chat 常驻列
 * （`AssistantColumn.vue`）共用本模块——**不重复建设会话体系**（DR-C8 §3）。
 *
 * 承载：会话列表 / 消息首屏与增量轮询 / 发送链路 / 响应卡写回 / 上下文锚点。
 * 不承载（留在会话页）：访谈段（DK-F discovery 段）、页面路由与版式。
 */

/** 增量轮询单次拉取条数 = 服务端 clamp 上限（MAX_MESSAGE_LIMIT）。 */
const MESSAGE_PAGE_LIMIT = 500;
/**
 * 首屏最新窗口条数（Batch H）：`latest=true, limit=200`——会话超过 200 条
 * 时首屏停在最新 200 条（ASC 返回），不再从最旧 500 条起。
 */
const FIRST_PAGE_LIMIT = 200;
/** 增量轮询间隔：assistant 回复生成以秒计，4s 是延迟与请求量的折中。 */
const POLL_INTERVAL_MS = 4000;
/** 自动建会话时从首条消息截取的 title 长度。 */
const TITLE_MAX_CHARS = 40;

/**
 * 后端信封 → 组件库 `interaction` 信封（Batch 3B）。
 *
 * **fail-closed**：类型不在 8 类白名单内 / 载荷缺必填字段 → 返回 null，
 * 该消息退化为**纯文本**（不留空白、不伪造卡片、不抛未处理异常）。
 */
export function toInteractionEnvelope(
  metadata: unknown
): AiInteractionEnvelope | undefined {
  if (!metadata || typeof metadata !== "object") return undefined;
  const envelope = (metadata as Record<string, unknown>).envelope;
  if (!envelope || typeof envelope !== "object") return undefined;

  const raw = envelope as Record<string, unknown>;
  const type = raw.type;
  if (
    typeof type !== "string" ||
    !(RESPONSE_TYPES as readonly string[]).includes(type)
  ) {
    return undefined;
  }
  const status = raw.status;
  if (
    typeof status !== "string" ||
    !(INTERACTION_STATUSES as readonly string[]).includes(status)
  ) {
    return undefined;
  }
  const payload = raw.payload;
  if (!payload || typeof payload !== "object") return undefined;
  const identity = raw.identity;
  if (!identity || typeof identity !== "object") return undefined;

  // payload 深校验（fail-closed 第二半）：必填字段缺一即整信封拒收——
  // 卡片组件对缺失 TS 必填字段会渲染崩，宁退纯文本不冒渲染崩风险。
  const payloadRecord = payload as Record<string, unknown>;
  const required = PAYLOAD_REQUIRED_FIELDS[type as AiResponseType] ?? [];
  for (const field of required) {
    if (!(field in payloadRecord)) return undefined;
  }

  return {
    id: typeof raw.id === "string" ? raw.id : "",
    type: type as AiResponseType,
    status: status as InteractionStatus,
    identity: identity as AiIdentity,
    createdAt:
      typeof raw.createdAt === "string"
        ? raw.createdAt
        : typeof raw.created_at === "string"
          ? raw.created_at
          : "",
    title: typeof raw.title === "string" ? raw.title : "",
    description:
      typeof raw.description === "string" ? raw.description : undefined,
    conclusion: typeof raw.conclusion === "string" ? raw.conclusion : undefined,
    evidence:
      raw.evidence && typeof raw.evidence === "object"
        ? (raw.evidence as AiEvidence)
        : undefined,
    payload: payload as Record<string, unknown>
  };
}

/**
 * 后端记录 → 组件库时间线消息。role 仅 user 走用户侧，其余一律 AI 侧。
 *
 * **信封消费**（Batch 3B）：消息 metadata 带 `envelope` → 组装 `interaction`
 * 内联渲染响应卡；无信封或信封非法 → 纯文本（AI 的普通消息保持轻量）。
 */
export function toTimelineMessage(
  record: ConversationMessageRecord
): ConversationMessage {
  const interaction = toInteractionEnvelope(record.metadata);
  return {
    id: record.id,
    role: record.role === "user" ? "user" : "ai",
    createdAt: record.created_at,
    text: record.content,
    ...(interaction ? { interaction } : {})
  };
}

/** 首条消息派生会话标题（压平空白后截断；后端允许空 title）。 */
function deriveTitle(text: string): string {
  return text.replace(/\s+/g, " ").trim().slice(0, TITLE_MAX_CHARS);
}

export interface UseAssistantConversationOptions {
  /** 生效的项目 id（会话按项目归属）。 */
  projectId: Ref<string>;
  /** 交接快照（指令栏提交时刻定格）；新会话建锚点时优先于活态上下文。 */
  pendingContext?: Ref<AssistantContextSnapshot | null>;
  /** 发送落点回调（DK-A：升级为访谈时同步链接 + 提示）。 */
  onLanded?: (outcome: SendMessageOutcome) => void;
  /** 会话切换 / 列表重载后的回调（访谈链接同步）。 */
  onConversationChanged?: (conversation: Conversation | null) => void;
  /** 每轮增量轮询结束后的回调（访谈段同节奏刷新）。 */
  onPollTick?: () => void;
  /** 会话无标题时的回落文案（会话切换器下拉用）。 */
  fallbackTitle?: string;
}

/**
 * 会话机制实例。返回状态 + 方法，供会话页与常驻列各自组装界面。
 */
export function useAssistantConversation(
  options: UseAssistantConversationOptions
) {
  const {
    projectId,
    pendingContext,
    onLanded,
    onConversationChanged,
    onPollTick,
    fallbackTitle = "—"
  } = options;
  const shell = useShellStore();

  const conversations = ref<Conversation[]>([]);
  const selectedId = ref("");
  const timeline = ref<ConversationMessage[]>([]);

  const loading = ref(true);
  const loadError = ref("");
  const messagesLoading = ref(false);
  const sending = ref(false);
  const sendError = ref("");
  const pendingText = ref("");
  /** 新会话草稿态：点了「新建会话」但尚未发出首条消息（不落库）。 */
  const drafting = ref(false);

  // ───────────────────────────────────────────────────────── 增量轮询

  let pollTimer: number | null = null;
  let pollInFlight = false;
  /** 卸载后置位：残留定时器回调直接短路，避免跨用例/跨页面脏请求。 */
  let disposed = false;

  function stopPolling() {
    if (pollTimer !== null) {
      window.clearTimeout(pollTimer);
      pollTimer = null;
    }
  }

  function schedulePolling() {
    if (disposed) return;
    stopPolling();
    pollTimer = window.setTimeout(() => {
      pollTimer = null;
      void pollMessages();
    }, POLL_INTERVAL_MS);
  }

  /**
   * 一次增量拉取：游标 = 时间线最后一条（任何 role）；空时间线退化为
   * 全量首屏。返回记录按 id 去重后追加（任何 role，assistant → AI 侧气泡）。
   * 任何失败静默吞掉（保留旧态，不打扰用户），成功且仍停在同一会话时续排
   * 下一轮；会话已切换则不自续（切会话路径自己负责新定时器）。
   */
  async function pollMessages() {
    const conversationId = selectedId.value;
    if (disposed || !projectId.value || !conversationId || pollInFlight) return;
    pollInFlight = true;
    try {
      const last = timeline.value.at(-1) ?? null;
      const query = last
        ? {
            after: last.createdAt,
            after_id: last.id,
            limit: MESSAGE_PAGE_LIMIT
          }
        : // 空时间线退化 = 与首屏一致的最新窗口（Batch H）。
          { latest: true, limit: FIRST_PAGE_LIMIT };
      const records = await listMessages(
        projectId.value,
        conversationId,
        query
      );
      if (selectedId.value !== conversationId) return;
      const known = new Set(timeline.value.map(message => message.id));
      // system 边界消息不进面板时间线（访谈分隔条另行渲染，DK-F）。
      const fresh = records
        .filter(record => !known.has(record.id) && record.role !== "system")
        .map(toTimelineMessage);
      if (fresh.length > 0) {
        timeline.value = [...timeline.value, ...fresh];
      }
    } catch {
      // 静默：轮询失败保留旧态（429 限流/网络抖动均不打扰用户）。
    } finally {
      pollInFlight = false;
    }
    // 访谈段与 chat 段同一节奏刷新（链接存在时；失败静默保留旧态）。
    onPollTick?.();
    if (selectedId.value === conversationId) schedulePolling();
  }

  async function loadMessages(conversationId: string) {
    messagesLoading.value = true;
    try {
      // 首屏最新窗口（Batch H）：latest=true, limit=200——服务端 DESC 取最新
      // 200 条再反转成 ASC；轮询增量仍走 after/after_id 不变。
      const records = await listMessages(projectId.value, conversationId, {
        latest: true,
        limit: FIRST_PAGE_LIMIT
      });
      if (selectedId.value !== conversationId) return;
      // system 边界消息不进面板时间线（访谈分隔条另行渲染，DK-F）。
      timeline.value = records
        .filter(record => record.role !== "system")
        .map(toTimelineMessage);
    } finally {
      messagesLoading.value = false;
    }
  }

  function onSelect(id: string) {
    drafting.value = false;
    if (!id || id === selectedId.value) return;
    // 切会话：先停旧轮询链，加载完成后续排新链（游标随时间线重置）。
    stopPolling();
    selectedId.value = id;
    sendError.value = "";
    pendingText.value = "";
    onConversationChanged?.(
      conversations.value.find(item => item.id === id) ?? null
    );
    void loadMessages(id)
      .catch(() => {})
      .finally(() => {
        if (selectedId.value === id) schedulePolling();
      });
  }

  async function load() {
    const target = projectId.value;
    if (!target) {
      stopPolling();
      loading.value = false;
      return;
    }
    stopPolling();
    loading.value = true;
    loadError.value = "";
    try {
      conversations.value = await listConversations(target);
      selectedId.value = conversations.value[0]?.id ?? "";
      timeline.value = [];
      onConversationChanged?.(conversations.value[0] ?? null);
      if (selectedId.value) await loadMessages(selectedId.value);
    } catch (error) {
      loadError.value =
        error instanceof Error ? error.message : String(error ?? "");
    } finally {
      loading.value = false;
      if (selectedId.value) schedulePolling();
    }
  }

  function reload() {
    void load();
  }

  /** 项目上下文变化 / 会话清空。 */
  function reset() {
    stopPolling();
    conversations.value = [];
    selectedId.value = "";
    timeline.value = [];
    onConversationChanged?.(null);
    loading.value = false;
    loadError.value = "";
  }

  // ───────────────────────────────────────────────────────── 发送链路

  /**
   * 响应卡真提交（Batch 3C）：卡片动作 → 写回端点权威落库。
   *
   * 动作映射：accept→accept / reject→reject(带 reason) / resolve→decision
   * （option_id + answer.confirmed 服务端复验）/ submit→submit（答案整体入
   * answer）。defer 是卡内本地态（组件库不触发 submit），不落端点（挂账）。
   *
   * 返回 SubmitResult 形状（卡的状态机消费）；端点 4xx/409 抛 Error →
   * 卡走错误相 + 重试入口（A-19）。写回成功后刷新本条消息的信封
   * （本地把终态/记录号同步进 timeline，与 A-21 回读一致）。
   */
  async function handleCardSubmit(
    payload: unknown,
    action?: string,
    messageId?: string
  ): Promise<{ recorded: boolean; reference?: string }> {
    const target = projectId.value;
    const conversationId = selectedId.value;
    if (!action || !messageId || !target || !conversationId) {
      throw new Error("interaction is missing context");
    }
    const source = (payload ?? {}) as Record<string, unknown>;
    const body: SubmitInteractionPayload = (() => {
      switch (action) {
        case "accept":
          return { action: "accept" };
        case "reject":
          return {
            action: "reject",
            reason:
              typeof source.reason === "string" && source.reason.trim()
                ? source.reason.trim()
                : undefined
          };
        case "resolve":
          return {
            action: "decision",
            option_id:
              typeof source.optionId === "string" ? source.optionId : undefined,
            answer: {
              confirmed: source.confirmedAsSelf === true,
              comment:
                typeof source.comment === "string" ? source.comment : undefined,
              optionLabel:
                typeof source.optionLabel === "string"
                  ? source.optionLabel
                  : undefined
            }
          };
        default:
          return { action: "submit", answer: payload };
      }
    })();

    try {
      const record = await submitInteraction(
        target,
        conversationId,
        messageId,
        body
      );
      // 信封本地终态化（与 A-21 服务端回读一致，免等待下一次轮询）。
      const index = timeline.value.findIndex(item => item.id === messageId);
      if (index >= 0 && timeline.value[index].interaction) {
        const message = timeline.value[index];
        timeline.value[index] = {
          ...message,
          interaction: {
            ...message.interaction!,
            status: record.status as InteractionStatus
            // SubmitResult 结果经信封 result 面向回读可见；卡内已展示记录号
          }
        };
      }
      return { recorded: true, reference: record.reference };
    } catch (error) {
      throw new Error(
        error instanceof Error
          ? error.message
          : String(error ?? "submit failed")
      );
    }
  }

  /**
   * 会话上下文锚点（Batch 2，0003）。
   *
   * 取**交接快照**（指令栏提交时刻定格）；页面内直接新建会话时取活态
   * assistantContext。锚点只声明「这个线程对着哪个项目/仓库说话」——服务端
   * 校验真实性（仓库/分支必须是本项目真实存在的），注入内容由服务端另行
   * 投影（Batch 3A）。
   */
  const contextAnchor = computed<ConversationContextAnchor | undefined>(() => {
    const snapshot = pendingContext?.value ?? shell.assistantContext;
    const anchor: ConversationContextAnchor = {};
    if (snapshot.projectId) anchor.project_id = snapshot.projectId;
    if (snapshot.projectName) anchor.project_name = snapshot.projectName;
    if (snapshot.repositoryName)
      anchor.repository_name = snapshot.repositoryName;
    if (snapshot.defaultBranch) anchor.default_branch = snapshot.defaultBranch;
    if (snapshot.repositoryRole)
      anchor.repository_role = snapshot.repositoryRole;
    return Object.keys(anchor).length ? anchor : undefined;
  });

  /**
   * 发送 = 等待 201 再插入（不乐观插入）：面板 emit send 即清空输入框，
   * 失败时原文进 pendingText 供「重发」，避免静默丢字。
   *
   * **项目落点取生效项目**（整合计划 Batch 1）：指令栏交接来的提问按提交
   * 时刻的快照项目发送，不受挂载后切项目影响。
   */
  async function handleSend(text: string) {
    const target = projectId.value;
    if (sending.value || !target) return;
    sending.value = true;
    sendError.value = "";
    try {
      let conversationId = selectedId.value;
      if (!conversationId) {
        const anchor = contextAnchor.value;
        const created = await createConversation(target, {
          title: deriveTitle(text),
          ...(anchor ? { context: anchor } : {})
        });
        conversations.value = [created, ...conversations.value];
        drafting.value = false;
        selectedId.value = created.id;
        timeline.value = [];
        conversationId = created.id;
      }
      const record = await appendMessage(target, conversationId, {
        content: text
      });
      // 落点判定（DK-A）：升级为访谈时同步链接 + 提示 + 立即拉访谈段；
      // 访谈落点的消息在 discovery 段（随访谈段拉取呈现），不进 chat 时间线。
      onLanded?.(record);
      if (record.landed !== "interview") {
        timeline.value = [...timeline.value, toTimelineMessage(record)];
      }
      pendingText.value = "";
      // 发送成功立即触发一次增量轮询：assistant 回复可能已落库
      //（去重保证与本地追加的 user 消息不冲突）。
      void pollMessages();
      // 会话行本地同步（列表按 updated_at DESC，发送后提前到首位）。
      const index = conversations.value.findIndex(
        item => item.id === conversationId
      );
      if (index > 0) {
        const [item] = conversations.value.splice(index, 1);
        conversations.value.unshift(item);
      }
      if (index >= 0) {
        const item = conversations.value[0];
        item.updated_at = record.created_at;
        item.last_message_at = record.created_at;
        // 落点回包携带的链接同步到会话行（切回该线程时访谈段直接可见）。
        if (record.discovery_session_id) {
          item.discovery_session_id = record.discovery_session_id;
        }
      }
    } catch (error) {
      sendError.value =
        error instanceof Error ? error.message : String(error ?? "");
      pendingText.value = text;
    } finally {
      sending.value = false;
    }
  }

  function resend() {
    if (!pendingText.value) return;
    void handleSend(pendingText.value);
  }

  /** 新建会话：立即可见（空时间线），首条消息发出时才落库。 */
  function startConversation() {
    stopPolling();
    drafting.value = true;
    selectedId.value = "";
    timeline.value = [];
    sendError.value = "";
    pendingText.value = "";
    onConversationChanged?.(null);
  }

  const selectedConversation = computed(
    () => conversations.value.find(item => item.id === selectedId.value) ?? null
  );

  /** 面板标题：选中会话用其 title；未选会话回落到组件库内置标题。 */
  const panelTitle = computed(() => selectedConversation.value?.title ?? "");

  const conversationOptions = computed(() =>
    conversations.value.map(item => ({
      value: item.id,
      label: item.title || fallbackTitle
    }))
  );

  onUnmounted(() => {
    disposed = true;
    stopPolling();
  });

  return {
    // 状态
    conversations,
    selectedId,
    timeline,
    loading,
    loadError,
    messagesLoading,
    sending,
    sendError,
    pendingText,
    drafting,
    // 派生
    selectedConversation,
    panelTitle,
    conversationOptions,
    contextAnchor,
    // 方法
    load,
    reload,
    reset,
    onSelect,
    handleSend,
    resend,
    handleCardSubmit,
    startConversation,
    stopPolling
  };
}
