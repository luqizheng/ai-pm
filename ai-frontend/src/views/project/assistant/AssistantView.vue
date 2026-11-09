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
import AiConversationMessage from "@/components/ui/ai-interaction/conversation/AiConversationMessage.vue";
import type { ConversationMessage } from "@/components/ui/ai-interaction/types";
import {
  appendMessage,
  createConversation,
  listConversations,
  listMessages,
  type Conversation,
  type ConversationMessage as ConversationMessageRecord,
  type SendMessageOutcome
} from "@/api/conversations";
import {
  getDiscoverySession,
  type DiscoverySessionDetail,
  type DiscoveryMessage
} from "@/api/discovery";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import { useShellStore } from "@/stores/shell";

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

// ───────────────────────────────────────────────────── 访谈段（DK-F 接线）
//
// 线程升级为访谈会话后（DK-A 载体升级），后续消息落 discovery 段
// （discovery_message，不在 conversation_message 里）——本页把时间线
// 拼成两段：chat 段（面板原样）+ 边界分隔条 + 访谈段 + 待办块。
// 链接真源 = 会话行的 discovery_session_id（发送落点回包同步更新）。

/** 选中线程当前链接的访谈会话 id；空 = 纯 chat 线程。 */
const interviewSessionId = ref("");
/** 访谈段数据（messages/findings/questions + session.status）。 */
const interview = ref<DiscoverySessionDetail | null>(null);
/** 升级时的服务端提示（handoff.notice，如「已切换到需求澄清」）。 */
const handoffNotice = ref("");

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
      ? { after: last.createdAt, after_id: last.id, limit: MESSAGE_PAGE_LIMIT }
      : // 空时间线退化 = 与首屏一致的最新窗口（Batch H）。
        { latest: true, limit: FIRST_PAGE_LIMIT };
    const records = await listMessages(projectId.value, conversationId, query);
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
  if (interviewSessionId.value) void loadInterview(interviewSessionId.value);
  if (selectedId.value === conversationId) schedulePolling();
}

// ────────────────────────────────────────────── 访谈段加载与映射

/** 拉访谈会话详情（messages/findings/questions）；失败静默保留旧态。 */
async function loadInterview(sessionId: string) {
  if (!projectId.value || !sessionId) return;
  try {
    const detail = await getDiscoverySession(projectId.value, sessionId);
    if (interviewSessionId.value === sessionId) interview.value = detail;
  } catch {
    // 静默：访谈段失败不打扰 chat 段。
  }
}

/** discovery 消息 → 组件库时间线消息（speaker user 走用户侧，其余 AI 侧）。 */
function toInterviewTimelineMessage(
  message: DiscoveryMessage
): ConversationMessage {
  return {
    id: message.id,
    role: message.speaker === "user" ? "user" : "ai",
    createdAt: message.created_at,
    text: message.content
  };
}

const interviewMessages = computed(() =>
  (interview.value?.messages ?? []).map(toInterviewTimelineMessage)
);

/** 待答问题（open；why_blocking 说明「不问清会导致什么」）。 */
const openQuestions = computed(
  () => interview.value?.questions.filter(q => q.status === "open") ?? []
);

/** 待知识应用（deferred；更正标记正文带 [更正待知识应用] 前缀）。 */
const deferredQuestions = computed(
  () => interview.value?.questions.filter(q => q.status === "deferred") ?? []
);

/** 已确认事实（人工裁决 CONFIRMED 的 finding）。 */
const confirmedFindings = computed(
  () =>
    interview.value?.findings.filter(f => f.verification_status === "CONFIRMED") ??
    []
);

/** 发送落点回包：升级为访谈时更新链接 + 提示，并立即拉一段访谈数据。 */
function applyLanding(outcome: SendMessageOutcome) {
  if (outcome.landed === "interview" && outcome.discovery_session_id) {
    interviewSessionId.value = outcome.discovery_session_id;
    handoffNotice.value = outcome.handoff?.notice ?? "";
    interview.value = null;
    void loadInterview(outcome.discovery_session_id);
  }
}

const selectedConversation = computed(
  () => conversations.value.find(item => item.id === selectedId.value) ?? null
);

const contextLabel = computed(() => {
  const project = scope.projects.find(item => item.id === projectId.value);
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

/** 后端记录 → 组件库时间线消息。role 仅 user 走用户侧，其余一律 AI 侧。 */
function toTimelineMessage(
  record: ConversationMessageRecord
): ConversationMessage {
  return {
    id: record.id,
    role: record.role === "user" ? "user" : "ai",
    createdAt: record.created_at,
    text: record.content
  };
}

async function loadMessages(conversationId: string) {
  messagesLoading.value = true;
  try {
    // 首屏最新窗口（Batch H）：latest=true, limit=200——服务端 DESC 取最新
    // 200 条再反转为 ASC；轮询增量仍走 after/after_id 不变。
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

/** 线程 → 访谈链接切换：重置访谈段并按会话行上的链接重拉。 */
function syncInterviewLink(conversation: Conversation | null) {
  interviewSessionId.value = conversation?.discovery_session_id ?? "";
  handoffNotice.value = "";
  interview.value = null;
  if (interviewSessionId.value) void loadInterview(interviewSessionId.value);
}

function onSelect(id: string) {
  drafting.value = false;
  if (!id || id === selectedId.value) return;
  // 切会话：先停旧轮询链，加载完成后续排新链（游标随时间线重置）。
  stopPolling();
  selectedId.value = id;
  sendError.value = "";
  pendingText.value = "";
  syncInterviewLink(conversations.value.find(item => item.id === id) ?? null);
  void loadMessages(id)
    .catch(() => {})
    .finally(() => {
      if (selectedId.value === id) schedulePolling();
    });
}

async function load() {
  if (!projectId.value) {
    stopPolling();
    loading.value = false;
    return;
  }
  stopPolling();
  loading.value = true;
  loadError.value = "";
  try {
    conversations.value = await listConversations(projectId.value);
    selectedId.value = conversations.value[0]?.id ?? "";
    timeline.value = [];
    syncInterviewLink(conversations.value[0] ?? null);
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
 * 发送 = 等待 201 再插入（不乐观插入）：面板 emit send 即清空输入框，
 * 失败时原文进 pendingText 供「重发」，避免静默丢字。
 */
async function handleSend(text: string) {
  if (sending.value || !projectId.value) return;
  sending.value = true;
  sendError.value = "";
  try {
    let conversationId = selectedId.value;
    if (!conversationId) {
      const created = await createConversation(projectId.value, {
        title: deriveTitle(text)
      });
      conversations.value = [created, ...conversations.value];
      drafting.value = false;
      selectedId.value = created.id;
      timeline.value = [];
      conversationId = created.id;
    }
    const record = await appendMessage(projectId.value, conversationId, {
      content: text
    });
    // 落点判定（DK-A）：升级为访谈时同步链接 + 提示 + 立即拉访谈段；
    // 访谈落点的消息在 discovery 段（随访谈段拉取呈现），不进 chat 时间线。
    applyLanding(record);
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

onMounted(() => {
  void load();
});

// 指令栏转来的提问（B-1：指令栏是会话的收起入口）。槽为对象（同文案
// 连续提交也能触发）；immediate 覆盖「先入队再进页」的导航时序。
watch(
  () => shell.pendingAssistantPrompt,
  pending => {
    if (pending) void sendAfterLoad(shell.takeAssistantPrompt()!.text);
  },
  { immediate: true }
);

/** 等待首屏数据就绪后再发送（避免与 load 的会话选择竞态） */
async function sendAfterLoad(text: string) {
  while (loading.value) {
    await new Promise(resolve => setTimeout(resolve, 20));
  }
  void handleSend(text);
}

/** 新建会话：立即可见（空时间线），首条消息发出时才落库。 */
function startConversation() {
  stopPolling();
  drafting.value = true;
  selectedId.value = "";
  timeline.value = [];
  sendError.value = "";
  pendingText.value = "";
  syncInterviewLink(null);
}

onUnmounted(() => {
  disposed = true;
  stopPolling();
});

// 项目上下文变化（顶栏切项目 / 测试环境路由晚于挂载落定）→ 重拉会话列表。
watch(projectId, next => {
  if (!next) {
    stopPolling();
    conversations.value = [];
    selectedId.value = "";
    timeline.value = [];
    syncInterviewLink(null);
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

        <div
          class="assistant-stage"
          :class="{ 'assistant-stage--split': interviewSessionId }"
        >
          <AiConversationPanel
            :messages="timeline"
            :title="panelTitle"
            :context-label="contextLabel"
            @send="handleSend"
          />
          <!-- DK-F 访谈段（§2.5 体例）：边界分隔条 + 两段中的 discovery 段 + 待办 -->
          <section
            v-if="interviewSessionId"
            class="assistant-interview"
            :aria-label="t('assistant.interviewSegmentLabel')"
          >
            <p class="assistant-interview__separator" role="separator">
              <span class="assistant-interview__separator-label">
                {{ handoffNotice || t("assistant.interviewNotice") }}
              </span>
              <span
                v-if="interview?.session.status"
                class="assistant-interview__status"
              >
                {{ t("assistant.interviewStatus") }}:
                {{ interview.session.status }}
              </span>
            </p>
            <div
              v-if="interviewMessages.length > 0"
              class="assistant-interview__timeline"
            >
              <AiConversationMessage
                v-for="message in interviewMessages"
                :key="message.id"
                :message="message"
              />
            </div>
            <div class="assistant-interview__todo">
              <h3 class="assistant-interview__todo-title">
                {{ t("assistant.todoTitle") }}
              </h3>
              <div
                v-if="openQuestions.length > 0"
                class="assistant-interview__group"
              >
                <h4 class="assistant-interview__group-title">
                  {{ t("assistant.openQuestions") }}
                  ({{ openQuestions.length }})
                </h4>
                <ul class="assistant-interview__list">
                  <li
                    v-for="question in openQuestions"
                    :key="question.id"
                    class="assistant-interview__item"
                  >
                    <span class="assistant-interview__item-text">
                      {{ question.question }}
                    </span>
                    <span
                      v-if="question.why_blocking"
                      class="assistant-interview__item-hint"
                    >
                      {{ t("assistant.whyBlockingPrefix") }}:
                      {{ question.why_blocking }}
                    </span>
                  </li>
                </ul>
              </div>
              <div
                v-if="deferredQuestions.length > 0"
                class="assistant-interview__group"
              >
                <h4 class="assistant-interview__group-title">
                  {{ t("assistant.pendingKnowledge") }}
                  ({{ deferredQuestions.length }})
                </h4>
                <ul class="assistant-interview__list">
                  <li
                    v-for="question in deferredQuestions"
                    :key="question.id"
                    class="assistant-interview__item"
                  >
                    <span class="assistant-interview__item-text">
                      {{ question.question }}
                    </span>
                  </li>
                </ul>
              </div>
              <div
                v-if="confirmedFindings.length > 0"
                class="assistant-interview__group"
              >
                <h4 class="assistant-interview__group-title">
                  {{ t("assistant.confirmedFacts") }}
                  ({{ confirmedFindings.length }})
                </h4>
                <ul class="assistant-interview__list">
                  <li
                    v-for="finding in confirmedFindings"
                    :key="finding.id"
                    class="assistant-interview__item"
                  >
                    <span class="assistant-interview__item-text">
                      {{ finding.content }}
                    </span>
                  </li>
                </ul>
              </div>
            </div>
          </section>
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

/* 访谈段分栏（DK-F）：chat 面板 + discovery 段上下排布；
   未链接时布局与既有完全一致（.assistant-stage--split 不出现）。 */
.assistant-stage--split {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.assistant-stage--split > * {
  height: auto;
  min-height: 0;
}

.assistant-stage--split > .ai-conversation {
  flex: 1 1 52%;
}

.assistant-interview {
  display: flex;
  flex: 1 1 48%;
  flex-direction: column;
  gap: var(--space-2);
  min-height: 0;
  padding: var(--space-3);
  overflow-y: auto;
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
}

.assistant-interview__separator {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-small);
}

.assistant-interview__separator-label {
  padding: var(--space-1) var(--space-3);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-full);
  color: var(--ws-text-primary);
  font-weight: var(--weight-medium);
}

.assistant-interview__status {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.assistant-interview__timeline {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.assistant-interview__todo {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-top: var(--space-2);
}

.assistant-interview__todo-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
}

.assistant-interview__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.assistant-interview__group-title {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-small);
  font-weight: var(--weight-medium);
}

.assistant-interview__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.assistant-interview__item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-1) var(--space-2);
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.assistant-interview__item-text {
  color: var(--ws-text-primary);
  font-size: var(--font-size-small);
}

.assistant-interview__item-hint {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}
</style>
