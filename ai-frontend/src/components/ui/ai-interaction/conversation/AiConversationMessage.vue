<script setup lang="ts">
import { computed, markRaw, type Component } from "vue";
import { Sparkles } from "lucide-vue-next";

import ActionRequestResponse from "../responses/ActionRequestResponse.vue";
import ArtifactResponse from "../responses/ArtifactResponse.vue";
import DecisionResponse from "../responses/DecisionResponse.vue";
import ExecutionResultResponse from "../responses/ExecutionResultResponse.vue";
import InformationalResponse from "../responses/InformationalResponse.vue";
import ProposalResponse from "../responses/ProposalResponse.vue";
import QuestionResponse from "../responses/QuestionResponse.vue";
import RecommendationResponse from "../responses/RecommendationResponse.vue";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import { defaultAiIdentity } from "../identity";
import type { AiResponseType, ConversationMessage } from "../types";

/**
 * 会话消息：用户消息靠右（次级面、无 AI 身份章），AI 消息靠左
 * （普通消息带头像章与时间；结构化交互内联渲染对应响应卡）。
 */
const props = defineProps<{
  message: ConversationMessage;
  /**
   * 真实提交通道（Batch 3C）：(payload, action, messageId)；缺省时各卡
   * 自行回落 mock（目录页/设计态）。
   */
  submit?: (
    payload: unknown,
    action?: string,
    messageId?: string
  ) => Promise<unknown>;
}>();

const CARD_COMPONENTS: Record<AiResponseType, Component> = {
  informational: markRaw(InformationalResponse),
  recommendation: markRaw(RecommendationResponse),
  proposal: markRaw(ProposalResponse),
  question: markRaw(QuestionResponse),
  decision: markRaw(DecisionResponse),
  artifact: markRaw(ArtifactResponse),
  "action-request": markRaw(ActionRequestResponse),
  "execution-result": markRaw(ExecutionResultResponse)
};

const isUser = computed(() => props.message.role === "user");
const interaction = computed(() => props.message.interaction);
const cardComponent = computed(() =>
  interaction.value ? CARD_COMPONENTS[interaction.value.type] : null
);
const identity = computed(
  () => interaction.value?.identity ?? defaultAiIdentity
);
const timeLabel = computed(() => formatTime(props.message.createdAt));

/** 内联卡绑定：density 固定 conversation，时间用信封时间 */
const cardBindings = computed(() => {
  const value = interaction.value;
  if (!value) return {};
  const bindings: Record<string, unknown> = {
    payload: value.payload,
    identity: value.identity,
    timestamp: formatTime(value.createdAt),
    density: "conversation" as const,
    initialStatus: value.status
  };
  if (props.submit) {
    bindings.submit = (payload: unknown, action?: string) =>
      props.submit!(payload, action, props.message.id);
  }
  return bindings;
});

function formatTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  return `${hours}:${minutes}`;
}
</script>

<template>
  <article
    class="ai-conversation-message"
    :class="isUser ? 'is-user' : 'is-ai'"
    :data-role="message.role"
  >
    <template v-if="isUser">
      <p v-if="message.text" class="ai-conversation-message__bubble">
        {{ message.text }}
      </p>
      <time
        class="ai-conversation-message__time"
        :datetime="message.createdAt"
        >{{ timeLabel }}</time
      >
    </template>

    <template v-else>
      <header
        v-if="!cardComponent && message.text"
        class="ai-conversation-message__identity"
      >
        <span class="ai-conversation-message__avatar" aria-hidden="true">
          <Sparkles :size="12" />
        </span>
        <span class="ai-conversation-message__name">{{ identity.name }}</span>
        <span v-if="identity.role" class="ai-conversation-message__role">{{
          identity.role
        }}</span>
        <time
          class="ai-conversation-message__time"
          :datetime="message.createdAt"
          >{{ timeLabel }}</time
        >
      </header>

      <!-- AI 文本按 Markdown 渲染（LLM 回复 content_type=markdown）；
           渲染完成前 / 渲染失败时以原文兜底，不留空白 -->
      <MarkdownViewer
        v-if="message.text"
        class="ai-conversation-message__text"
        :model-value="message.text"
      >
        <template #empty>
          <p class="ai-conversation-message__plain">{{ message.text }}</p>
        </template>
      </MarkdownViewer>

      <component
        :is="cardComponent"
        v-if="cardComponent"
        class="ai-conversation-message__card"
        v-bind="cardBindings"
      />
    </template>
  </article>
</template>

<style scoped>
.ai-conversation-message {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  max-width: var(--size-ai-conversation-max-width);
  min-width: 0;
}

.ai-conversation-message.is-ai {
  align-self: flex-start;
  align-items: flex-start;
}

.ai-conversation-message.is-user {
  align-self: flex-end;
  align-items: flex-end;
  max-width: var(--size-ai-card-compact-width);
}

/* 用户消息：次级面 + 无 AI 身份章 */
.ai-conversation-message__bubble {
  max-width: 100%;
  margin: 0;
  padding: var(--space-2) var(--space-3);
  color: var(--ws-text-primary);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  white-space: pre-wrap;
  word-break: break-word;
}

/* AI 身份章（普通消息） */
.ai-conversation-message__identity {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.ai-conversation-message__avatar {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--size-ai-avatar);
  height: var(--size-ai-avatar);
  color: var(--ws-accent);
  background: var(--ws-color-accent-surface);
  border-radius: var(--radius-xs);
}

.ai-conversation-message__name {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  white-space: nowrap;
}

.ai-conversation-message__role {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

.ai-conversation-message__time {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

/* AI 文本消息：只读 Markdown 预览（MarkdownViewer），不走编辑器 */
.ai-conversation-message__text {
  max-width: 100%;
  min-width: 0;
  color: var(--ws-text-primary);
  word-break: break-word;
}

.ai-conversation-message__text :deep(.md-viewer__body > :last-child) {
  margin-bottom: 0;
}

.ai-conversation-message__plain {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  white-space: pre-wrap;
}

.ai-conversation-message__card {
  width: 100%;
  min-width: 0;
  max-width: var(--size-ai-card-conversation-width);
}

@media (prefers-reduced-motion: reduce) {
  .ai-conversation-message__bubble {
    transition: none;
  }
}
</style>
