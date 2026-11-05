<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Pin, Sparkles } from "lucide-vue-next";

import type { ConversationMessage } from "../types";
import AiConversationComposer from "./AiConversationComposer.vue";
import AiConversationMessage from "./AiConversationMessage.vue";

/**
 * 会话面板：标题 + 固定上下文提示 + 可滚消息时间线 + 固定底部输入条。
 *
 * 消息里只有结构化交互升级为卡片；卡片随会话宽度自适应（conversation 密度）。
 */
const props = withDefaults(
  defineProps<{
    messages: ConversationMessage[];
    title?: string;
    contextLabel?: string;
  }>(),
  {
    title: "",
    contextLabel: ""
  }
);

const emit = defineEmits<{ send: [text: string] }>();

const { t } = useI18n();

const draft = ref("");
const heading = computed(() => props.title || t("ai.conversation.title"));

function onSend(text: string): void {
  emit("send", text);
  draft.value = "";
}
</script>

<template>
  <section class="ai-conversation" :aria-label="heading">
    <header class="ai-conversation__header">
      <span class="ai-conversation__mark" aria-hidden="true">
        <Sparkles :size="14" />
      </span>
      <div class="ai-conversation__heading">
        <h2 class="ai-conversation__title">{{ heading }}</h2>
        <p class="ai-conversation__pinned">
          <Pin :size="12" aria-hidden="true" />
          <span>{{ t("ai.conversation.pinned") }}</span>
          <span v-if="contextLabel" class="ai-conversation__pinned-value">{{
            contextLabel
          }}</span>
        </p>
      </div>
    </header>

    <div class="ai-conversation__scroll">
      <div class="ai-conversation__timeline" role="log" aria-live="polite">
        <AiConversationMessage
          v-for="message in messages"
          :key="message.id"
          :message="message"
        />
      </div>
    </div>

    <AiConversationComposer
      v-model="draft"
      :context-label="contextLabel"
      @send="onSend"
    />
  </section>
</template>

<style scoped>
.ai-conversation {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  background: var(--ws-canvas);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
}

.ai-conversation__header {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-5);
  background: var(--ws-surface);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-conversation__mark {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--size-ai-avatar-lg);
  height: var(--size-ai-avatar-lg);
  color: var(--ws-accent);
  background: var(--ws-color-accent-surface);
  border-radius: var(--radius-xs);
}

.ai-conversation__heading {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.ai-conversation__title {
  margin: 0;
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
  line-height: var(--line-height-body);
}

.ai-conversation__pinned {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
  min-width: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-conversation__pinned > svg {
  flex: none;
}

.ai-conversation__pinned-value {
  overflow: hidden;
  color: var(--ws-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-conversation__scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-5);
}

.ai-conversation__timeline {
  display: flex;
  flex-direction: column;
  gap: var(--size-ai-message-gap);
  width: 100%;
  max-width: var(--size-ai-conversation-max-width);
  margin: 0 auto;
}
</style>
