<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { Plus, RefreshCw } from "lucide-vue-next";
import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Select from "@/components/ui/select/Select.vue";
import { PageFrame } from "@/components/shell";
import { AiConversationPanel } from "@/components/ui/ai-interaction/conversation";
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
import {
  appendMessage,
  createConversation,
  listConversations,
  listMessages,
  submitInteraction,
  type Conversation,
  type ConversationContextAnchor,
  type ConversationMessage as ConversationMessageRecord,
  type SubmitInteractionPayload
} from "@/api/conversations";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import { useShellStore, type AssistantContextSnapshot } from "@/stores/shell";

/**
 * Project AI conversation page（V2 D-4 会话 REST + Batch G assistant 回复）。
 *
 * 数据 = pm-api `/v1/pm/projects/{id}/conversations*`（`@/api/conversations`）：
 *   - 进入页面拉本人会话列表（updated_at DESC）；空列表不阻塞输入——
 *     首次发送自动建会话（title 取首条消息截断）再落消息；
 *   - 选中会话 → 拉消息：**首屏最新窗口**（Batch H）——`latest=true`，
 *     limit 200（服务端 DESC 取最新 200 条再反转为 ASC），会话很长时首屏
 *     停在最新尾部而不是最旧 500 条；更早历史暂不做回翻（挂账）；
 *   - **增量轮询**（Batch G）：选中会话后每 POLL_INTERVAL_MS 用
 *     `after/after_id`（最后一条消息）拉新消息并追加（任何 role——assistant
 *     回复由 ai-pm-worker 的会话回复链落库）；发送成功后立即触发一次；
 *     时间线为空时轮询退化为与首屏相同的最新窗口；切会话/项目切换/卸载
 *     清理定时器；轮询失败（429/网络抖动）静默保留旧态；
 *   - 发送链路：等待 201 后按返回记录插入本地时间线（不乐观插入），
 *     失败保留原文提供一键重发，不静默丢弃；
 *   - assistant 消息按返回 role 渲染到 AI 侧，内容一律来自服务端，不伪造；
 *   - 指令栏交接（B-1 混合工作区）：底部 AI 指令栏提交的提问入队到
 *     shell store，本页挂载时取出走既有发送链（自动建会话同样适用），
 *     消费即清空，不重复发送。
 */
const route = useRoute();
const { t } = useI18n();
const scope = useScopeStore();
const shell = useShellStore();

/** 新会话草稿态：点了「新建会话」但尚未发出首条消息（不落库）。 */
const drafting = ref(false);

const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  // scope store 缺省为 SCOPE_ALL（“全部项目”哨兵）：会话按项目归属，
  // 哨兵与空值都不当作 project id（历史值 "all" 一并兜住）。
  const scoped = scope.currentProjectId;
  if (scoped && scoped !== SCOPE_ALL && scoped !== "all") return scoped;
  return "";
});

/**
 * 指令栏交接的上下文快照（整合计划 Batch 1）。
 *
 * 提交时刻定格——「问的是哪个项目」由快照回答，而非页面挂载时的活态项目，
 * 避免导航途中切项目导致问题落到别的项目。
 */
const pendingContext = ref<AssistantContextSnapshot | null>(null);

/**
 * 生效的项目上下文。
 *
 * 交接窗口内以快照项目为准（问的是它，就落到它）；**发送完成后**释放快照、
 * 回落到活态项目——快照只管交接那一段，不锁死整页。
 */
const effectiveProjectId = computed(
  () => pendingContext.value?.projectId ?? projectId.value
);

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

const conversations = ref<Conversation[]>([]);
const selectedId = ref("");
const timeline = ref<ConversationMessage[]>([]);

const loading = ref(true);
const loadError = ref("");
const messagesLoading = ref(false);
const sending = ref(false);
const sendError = ref("");
const pendingText = ref("");

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
  if (disposed || !effectiveProjectId.value || !conversationId || pollInFlight)
    return;
  pollInFlight = true;
  try {
    const last = timeline.value.at(-1) ?? null;
    const query = last
      ? { after: last.createdAt, after_id: last.id, limit: MESSAGE_PAGE_LIMIT }
      : // 空时间线退化 = 与首屏一致的最新窗口（Batch H）。
        { latest: true, limit: FIRST_PAGE_LIMIT };
    const records = await listMessages(
      effectiveProjectId.value,
      conversationId,
      query
    );
    if (selectedId.value !== conversationId) return;
    const known = new Set(timeline.value.map(message => message.id));
    const fresh = records
      .filter(record => !known.has(record.id))
      .map(toTimelineMessage);
    if (fresh.length > 0) {
      timeline.value = [...timeline.value, ...fresh];
    }
  } catch {
    // 静默：轮询失败保留旧态（429 限流/网络抖动均不打扰用户）。
  } finally {
    pollInFlight = false;
  }
  if (selectedId.value === conversationId) schedulePolling();
}

const selectedConversation = computed(
  () => conversations.value.find(item => item.id === selectedId.value) ?? null
);

const contextLabel = computed(() => {
  const project = scope.projects.find(
    item => item.id === effectiveProjectId.value
  );
  return project?.name ?? "";
});

/** 面板标题：选中会话用其 title；未选会话回落到组件库内置标题。 */
const panelTitle = computed(() => selectedConversation.value?.title ?? "");

const conversationOptions = computed(() =>
  conversations.value.map(item => ({
    value: item.id,
    label: item.title || t("assistant.untitled")
  }))
);

// ───────────────────────────────────────────────────────── 数据加载

/**
 * 后端信封 → 组件库 `interaction` 信封（Batch 3B）。
 *
 * **fail-closed**：类型不在 8 类白名单内 / 载荷缺必填字段 → 返回 null，
 * 该消息退化为**纯文本**（不留空白、不伪造卡片、不抛未处理异常）。
 */
function toInteractionEnvelope(
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
function toTimelineMessage(
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

async function loadMessages(conversationId: string) {
  messagesLoading.value = true;
  try {
    // 首屏最新窗口（Batch H）：latest=true, limit=200——服务端 DESC 取最新
    // 200 条再反转为 ASC；轮询增量仍走 after/after_id 不变。
    const records = await listMessages(
      effectiveProjectId.value,
      conversationId,
      {
        latest: true,
        limit: FIRST_PAGE_LIMIT
      }
    );
    if (selectedId.value !== conversationId) return;
    timeline.value = records.map(toTimelineMessage);
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
  void loadMessages(id)
    .catch(() => {})
    .finally(() => {
      if (selectedId.value === id) schedulePolling();
    });
}

async function load() {
  const target = effectiveProjectId.value;
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

// ───────────────────────────────────────────────────────── 发送链路

/** 首条消息派生会话标题（压平空白后截断；后端允许空 title）。 */
function deriveTitle(text: string): string {
  return text.replace(/\s+/g, " ").trim().slice(0, TITLE_MAX_CHARS);
}

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
  const target = effectiveProjectId.value;
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
      error instanceof Error ? error.message : String(error ?? "submit failed")
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
  const snapshot = pendingContext.value ?? shell.assistantContext;
  const anchor: ConversationContextAnchor = {};
  if (snapshot.projectId) anchor.project_id = snapshot.projectId;
  if (snapshot.projectName) anchor.project_name = snapshot.projectName;
  if (snapshot.repositoryName) anchor.repository_name = snapshot.repositoryName;
  if (snapshot.defaultBranch) anchor.default_branch = snapshot.defaultBranch;
  if (snapshot.repositoryRole) anchor.repository_role = snapshot.repositoryRole;
  return Object.keys(anchor).length ? anchor : undefined;
});

/**
 * 发送 = 等待 201 再插入（不乐观插入）：面板 emit send 即清空输入框，
 * 失败时原文进 pendingText 供「重发」，避免静默丢字。
 *
 * **项目落点取 `effectiveProjectId`**（整合计划 Batch 1）：指令栏交接来的
 * 提问按提交时刻的快照项目发送，不受页面挂载后切项目影响。
 */
async function handleSend(text: string) {
  const target = effectiveProjectId.value;
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
    timeline.value = [...timeline.value, toTimelineMessage(record)];
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

onMounted(() => {
  void load();
});

// 指令栏转来的提问（B-1：指令栏是会话的收起入口）。槽为对象（同文案
// 连续提交也能触发）；immediate 覆盖「先入队再进页」的导航时序。
//
// **上下文快照随交接一并取走**（整合计划 Batch 1）：提交时刻的项目/仓库
// 上下文在此定格，随后建会话与发送都以此为准，不受页面挂载后切项目影响。
watch(
  () => shell.pendingAssistantPrompt,
  pending => {
    if (!pending) return;
    const taken = shell.takeAssistantPrompt();
    if (!taken) return;
    pendingContext.value = taken.context;
    void sendAfterLoad(taken.text);
  },
  { immediate: true }
);

/** 等待首屏数据就绪后再发送（避免与 load 的会话选择竞态） */
async function sendAfterLoad(text: string) {
  while (loading.value) {
    await new Promise(resolve => setTimeout(resolve, 20));
  }
  await handleSend(text);
  // **交接窗口结束**：释放快照，后续发送回落到活态项目
  // （快照只管交接那一段，不锁死整页——整合计划 Batch 1）。
  pendingContext.value = null;
}

/** 新建会话：立即可见（空时间线），首条消息发出时才落库。 */
function startConversation() {
  stopPolling();
  drafting.value = true;
  selectedId.value = "";
  timeline.value = [];
  sendError.value = "";
  pendingText.value = "";
}

onUnmounted(() => {
  disposed = true;
  stopPolling();
});

// 项目上下文变化（顶栏切项目 / 交接快照释放 / 路由晚于挂载落定）→ 重拉会话列表。
//
// 跟随 **effectiveProjectId** 而非活态 projectId：交接窗口内快照项目优先，
// 快照释放后若与活态项目不同，列表自动跟回活态项目（不滞留旧会话）。
watch(effectiveProjectId, next => {
  if (!next) {
    stopPolling();
    conversations.value = [];
    selectedId.value = "";
    timeline.value = [];
    loading.value = false;
    loadError.value = "";
    return;
  }
  void load();
});
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('pm.projects.pageTitle'), to: '/projects' }]"
    :title="t('assistant.pageTitle')"
    :description="t('assistant.subtitle')"
  >
    <Alert v-if="loadError" variant="danger" :title="t('assistant.loadError')">
      <span>{{ loadError }}</span>
      <Button variant="secondary" size="sm" @click="reload">
        <RefreshCw aria-hidden="true" />
        {{ t("assistant.loadRetry") }}
      </Button>
    </Alert>

    <EmptyState
      v-if="!projectId"
      :title="t('assistant.noProjectTitle')"
      :description="t('assistant.noProjectDesc')"
    />

    <template v-else>
      <EmptyState
        v-if="loading && conversations.length === 0"
        :title="t('assistant.loadingTitle')"
        :description="t('assistant.loadingDesc')"
      />

      <template v-else>
        <Alert
          v-if="conversations.length === 0"
          variant="info"
          :title="t('assistant.emptyGuide')"
        />

        <div
          v-if="conversations.length > 0 || drafting"
          class="assistant-switcher"
        >
          <label class="assistant-switcher__label" for="assistant-conversation">
            {{ t("assistant.switcherLabel") }}
          </label>
          <Select
            id="assistant-conversation"
            class="assistant-switcher__select"
            :model-value="selectedId"
            :options="conversationOptions"
            :placeholder="t('assistant.newPlaceholder')"
            :aria-label="t('assistant.switcherLabel')"
            @update:model-value="onSelect"
          />
          <Button
            v-if="conversations.length > 0 && !drafting"
            variant="secondary"
            size="sm"
            class="assistant-new"
            @click="startConversation"
          >
            <Plus aria-hidden="true" />
            {{ t("assistant.newConversation") }}
          </Button>
          <span
            v-if="messagesLoading"
            class="assistant-switcher__status"
            role="status"
          >
            {{ t("assistant.messagesLoading") }}
          </span>
        </div>

        <div class="assistant-stage">
          <AiConversationPanel
            :messages="timeline"
            :title="panelTitle"
            :context-label="contextLabel"
            :submit="handleCardSubmit"
            @send="handleSend"
          />
        </div>

        <Alert
          v-if="sendError"
          variant="danger"
          :title="t('assistant.sendError')"
        >
          <span>{{ sendError }}</span>
          <Button
            v-if="pendingText"
            variant="secondary"
            size="sm"
            @click="resend"
          >
            {{ t("assistant.resend") }}
          </Button>
        </Alert>

        <Alert
          v-if="selectedId"
          variant="info"
          :title="t('assistant.recordedNotice')"
        />
      </template>
    </template>
  </PageFrame>
</template>

<style scoped>
.assistant-switcher {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
  margin-bottom: var(--space-4);
}

.assistant-switcher__label {
  flex: none;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

.assistant-switcher__select {
  width: var(--size-ai-conversation-max-width);
  max-width: 100%;
}

.assistant-switcher__status {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.assistant-stage {
  /* 舞台高度：视口减壳顶栏/命令条/页框与切换器留白（尺寸全部走 token） */
  height: calc(
    100vh - var(--ws-topbar-height) - var(--ws-commandbar-height-min) -
      var(--space-16) - var(--space-16) - var(--space-10)
  );
  min-height: var(--size-ai-artifact-preview-height);
  overflow: hidden;
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
}

.assistant-stage > * {
  height: 100%;
}
</style>
