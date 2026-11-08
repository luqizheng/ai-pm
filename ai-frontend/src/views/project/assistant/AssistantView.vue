<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";
import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Select from "@/components/ui/select/Select.vue";
import { PageFrame } from "@/components/shell";
import { AiConversationPanel } from "@/components/ui/ai-interaction/conversation";
import type { ConversationMessage } from "@/components/ui/ai-interaction/types";
import {
  appendMessage,
  createConversation,
  listConversations,
  listMessages,
  type Conversation,
  type ConversationMessage as ConversationMessageRecord
} from "@/api/conversations";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";

/**
 * Project AI conversation page（V2 D-4 会话 REST 接线批，2026-11-08）。
 *
 * 数据 = pm-api `/v1/pm/projects/{id}/conversations*`（`@/api/conversations`）：
 *   - 进入页面拉本人会话列表（updated_at DESC）；空列表不阻塞输入——
 *     首次发送自动建会话（title 取首条消息截断）再落消息；
 *   - 选中会话 → 拉消息（created_at ASC，limit 500 = 服务端 clamp 上限；
 *     keyset `before` 仅能向前翻页，而首屏无「从最新往前」游标，故
 *     >500 条的历史尾巴暂不可达——后端分页形状限制，不伪造完整假象）；
 *   - 发送链路：等待 201 后按返回记录插入本地时间线（不乐观插入），
 *     失败保留原文提供一键重发，不静默丢弃；
 *   - 诚实缺位：assistant 消息尚无生产者（内部 AI 链后续批次），发送成功
 *     后仅展示明确的系统提示行，绝不伪造任何 AI 回复内容。
 */
const route = useRoute();
const { t } = useI18n();
const scope = useScopeStore();

const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  // scope store 缺省为 SCOPE_ALL（“全部项目”哨兵）：会话按项目归属，
  // 哨兵与空值都不当作 project id（历史值 "all" 一并兜住）。
  const scoped = scope.currentProjectId;
  if (scoped && scoped !== SCOPE_ALL && scoped !== "all") return scoped;
  return "";
});

/** 单次拉取条数 = 服务端 clamp 上限（MAX_MESSAGE_LIMIT）。 */
const MESSAGE_PAGE_LIMIT = 500;
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
    const records = await listMessages(projectId.value, conversationId, {
      limit: MESSAGE_PAGE_LIMIT
    });
    if (selectedId.value !== conversationId) return;
    timeline.value = records.map(toTimelineMessage);
  } finally {
    messagesLoading.value = false;
  }
}

function onSelect(id: string) {
  if (!id || id === selectedId.value) return;
  selectedId.value = id;
  sendError.value = "";
  pendingText.value = "";
  void loadMessages(id);
}

async function load() {
  if (!projectId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = "";
  try {
    conversations.value = await listConversations(projectId.value);
    selectedId.value = conversations.value[0]?.id ?? "";
    timeline.value = [];
    if (selectedId.value) await loadMessages(selectedId.value);
  } catch (error) {
    loadError.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
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
      selectedId.value = created.id;
      timeline.value = [];
      conversationId = created.id;
    }
    const record = await appendMessage(projectId.value, conversationId, {
      content: text
    });
    timeline.value = [...timeline.value, toTimelineMessage(record)];
    pendingText.value = "";
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

// 项目上下文变化（顶栏切项目 / 测试环境路由晚于挂载落定）→ 重拉会话列表。
watch(projectId, next => {
  if (!next) {
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

        <div v-if="conversations.length > 0" class="assistant-switcher">
          <label class="assistant-switcher__label" for="assistant-conversation">
            {{ t("assistant.switcherLabel") }}
          </label>
          <Select
            id="assistant-conversation"
            class="assistant-switcher__select"
            :model-value="selectedId"
            :options="conversationOptions"
            :placeholder="t('assistant.switcherPlaceholder')"
            :aria-label="t('assistant.switcherLabel')"
            @update:model-value="onSelect"
          />
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
