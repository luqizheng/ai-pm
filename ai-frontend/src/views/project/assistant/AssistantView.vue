<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
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
  type Conversation,
  type SendMessageOutcome
} from "@/api/conversations";
import {
  getDiscoverySession,
  transitionDiscoverySession,
  type DiscoverySessionDetail,
  type DiscoveryMessage
} from "@/api/discovery";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import { useShellStore, type AssistantContextSnapshot } from "@/stores/shell";
import { useAssistantConversation } from "@/composables/useAssistantConversation";

/**
 * Project AI conversation page（V2 D-4 会话 REST + Batch G assistant 回复）。
 *
 * **会话机制已抽到 `@/composables/useAssistantConversation`**（DR-C8 第 ② 步）：
 * 本页与 AI Chat 常驻列共用同一份会话实现——列表 / 消息首屏 / 增量轮询 /
 * 发送链路 / 响应卡写回 / 上下文锚点，**不重复建设会话体系**。
 *
 * 本页保留：访谈段（DK-F discovery 段）、指令栏交接窗口、项目上下文与版式。
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

/** 拉访谈会话详情（messages/findings/questions）；失败静默保留旧态。 */
async function loadInterview(sessionId: string) {
  if (!effectiveProjectId.value || !sessionId) return;
  try {
    const detail = await getDiscoverySession(
      effectiveProjectId.value,
      sessionId
    );
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

/** 访谈段时间线 → 组件库时间线消息（discovery 段呈现）。 */
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
    interview.value?.findings.filter(
      f => f.verification_status === "CONFIRMED"
    ) ?? []
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

/** 线程 → 访谈链接切换：重置访谈段并按会话行上的链接重拉。 */
function syncInterviewLink(conversation: Conversation | null) {
  interviewSessionId.value = conversation?.discovery_session_id ?? "";
  handoffNotice.value = "";
  interview.value = null;
  if (interviewSessionId.value) void loadInterview(interviewSessionId.value);
}

// ─────────────────────────────────────── 访谈人工处理入口（B03-01 死信修复）
//
// REVIEW_REQUIRED 的访谈不在编排扫描集内：消息落进去后 AI 不会消费，直到
// 人工退回（IN_PROGRESS，恢复编排）或完结（COMPLETED，回落 chat）。没有
// 这组入口时消息就是死信——这是 v1.0 验收 B03-01 的 P1 缺陷本体。

/** 访谈在等人工复核（REVIEW_REQUIRED）：AI 暂停消费，需要退回或完结。 */
const interviewAwaitingReview = computed(
  () => interview.value?.session.status === "REVIEW_REQUIRED"
);

/** 访谈在等人输入（NEEDS_USER_INPUT）：答完可人工完结。 */
const interviewNeedsUserInput = computed(
  () => interview.value?.session.status === "NEEDS_USER_INPUT"
);

/** 人工处理动作进行中（防重复提交；按钮 disabled）。 */
const transitioning = ref(false);
/** 跃迁失败提示（409=状态已被别处推进，刷新对齐而非报错打断）。 */
const transitionConflict = ref("");

/**
 * 人工处理访谈会话：退回继续（IN_PROGRESS）或复核通过并完结（COMPLETED）。
 * 完成后重拉访谈段；失败（含 409 竞态）也重拉——服务端状态是唯一真源。
 */
async function transitionInterview(status: "IN_PROGRESS" | "COMPLETED") {
  const sessionId = interviewSessionId.value;
  if (!effectiveProjectId.value || !sessionId || transitioning.value) return;
  transitioning.value = true;
  transitionConflict.value = "";
  try {
    await transitionDiscoverySession(effectiveProjectId.value, sessionId, status);
  } catch {
    transitionConflict.value = t("assistant.transitionConflict");
  } finally {
    transitioning.value = false;
    void loadInterview(sessionId);
  }
}

// ──────────────────────────────────── 会话机制（与 AI Chat 常驻列共用同一实现）
//
// DR-C8 第 ② 步：会话能力从本页抽到 composable，常驻列复用同一份——
// 列表 / 首屏 / 增量轮询 / 发送 / 响应卡写回 / 上下文锚点均在此，
// 本页只保留访谈段与交接窗口的页面级编排。

const {
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
  panelTitle,
  conversationOptions,
  load,
  reload,
  reset,
  onSelect,
  handleSend,
  resend,
  handleCardSubmit,
  startConversation
} = useAssistantConversation({
  projectId: effectiveProjectId,
  pendingContext,
  onLanded: applyLanding,
  onConversationChanged: syncInterviewLink,
  onPollTick: () => {
    if (interviewSessionId.value) void loadInterview(interviewSessionId.value);
  },
  fallbackTitle: t("assistant.untitled")
});

const contextLabel = computed(() => {
  const project = scope.projects.find(
    item => item.id === effectiveProjectId.value
  );
  return project?.name ?? "";
});

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

// 项目上下文变化（顶栏切项目 / 交接快照释放 / 路由晚于挂载落定）→ 重拉会话列表。
//
// 跟随 **effectiveProjectId** 而非活态 projectId：交接窗口内快照项目优先，
// 快照释放后若与活态项目不同，列表自动跟回活态项目（不滞留旧会话）。
watch(effectiveProjectId, next => {
  if (!next) {
    reset();
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
            :submit="handleCardSubmit"
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
            <!-- B03-01：编排轮失败对用户可见（系统自动重试，不伪造成功）。 -->
            <p
              v-if="interview?.session.last_round_error"
              class="assistant-interview__error"
              role="status"
              data-testid="interview-round-error"
            >
              {{ t("assistant.lastRoundError") }}:
              {{ interview.session.last_round_error }}
            </p>
            <!-- B03-01：等待人工复核/输入的显式提示 + 处理入口。 -->
            <div
              v-if="interviewAwaitingReview || interviewNeedsUserInput"
              class="assistant-interview__review"
              :class="{
                'assistant-interview__review--review': interviewAwaitingReview
              }"
              role="alert"
              data-testid="interview-review-banner"
            >
              <p class="assistant-interview__review-text">
                {{
                  interviewAwaitingReview
                    ? t("assistant.reviewRequiredNotice")
                    : t("assistant.needsInputNotice")
                }}
              </p>
              <div class="assistant-interview__review-actions">
                <Button
                  v-if="interviewAwaitingReview"
                  variant="secondary"
                  size="sm"
                  :disabled="transitioning"
                  data-testid="interview-resume"
                  @click="transitionInterview('IN_PROGRESS')"
                >
                  {{ t("assistant.resumeInterview") }}
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  :disabled="transitioning"
                  data-testid="interview-complete"
                  @click="transitionInterview('COMPLETED')"
                >
                  {{ t("assistant.completeInterview") }}
                </Button>
              </div>
              <p
                v-if="transitionConflict"
                class="assistant-interview__review-conflict"
              >
                {{ transitionConflict }}
              </p>
            </div>
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

/* B03-01：编排失败提示（自动重试中，非阻断）。 */
.assistant-interview__error {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

/* B03-01：等待人工复核/输入的处理面板。 */
.assistant-interview__review {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
}

.assistant-interview__review--review {
  border-color: var(--ws-color-warning);
}

.assistant-interview__review-text {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-small);
}

.assistant-interview__review-actions {
  display: flex;
  gap: var(--space-2);
}

.assistant-interview__review-conflict {
  margin: 0;
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
