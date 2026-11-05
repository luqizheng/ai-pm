<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowUp, Paperclip, Pin } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import IconButton from "@/components/ui/icon-button/IconButton.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

/**
 * 会话固定输入条：上下文芯片 + 多行输入 + 附件入口 + 发送。
 *
 * Enter 发送、Shift+Enter 换行；空值与禁用态不发请求；
 * 附件按钮只做入口占位（无实际上传）。
 */
const props = withDefaults(
  defineProps<{
    modelValue?: string;
    contextLabel?: string;
    disabled?: boolean;
    placeholder?: string;
  }>(),
  {
    modelValue: "",
    contextLabel: "",
    disabled: false,
    placeholder: undefined
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  send: [text: string];
}>();

const { t } = useI18n();

const value = computed({
  get: () => props.modelValue,
  set: (next: string) => emit("update:modelValue", next)
});

const canSend = computed(
  () => !props.disabled && value.value.trim().length > 0
);
const placeholderText = computed(
  () => props.placeholder || t("ai.conversation.composer.placeholder")
);

function submit(): void {
  if (!canSend.value) return;
  emit("send", value.value.trim());
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  submit();
}
</script>

<template>
  <div class="ai-conversation-composer" :class="{ 'is-disabled': disabled }">
    <p v-if="contextLabel" class="ai-conversation-composer__context">
      <Pin :size="12" aria-hidden="true" />
      <span class="ai-conversation-composer__context-label">{{
        t("ai.conversation.pinned")
      }}</span>
      <span class="ai-conversation-composer__context-value">{{
        contextLabel
      }}</span>
    </p>

    <div class="ai-conversation-composer__field" @keydown="onKeydown">
      <Textarea
        v-model="value"
        class="ai-conversation-composer__input"
        :rows="2"
        :disabled="disabled"
        :placeholder="placeholderText"
      />
      <div class="ai-conversation-composer__actions">
        <IconButton
          class="ai-conversation-composer__attach"
          variant="ghost"
          size="sm"
          :label="t('ai.conversation.composer.attach')"
          :disabled="disabled"
        >
          <Paperclip :size="14" aria-hidden="true" />
        </IconButton>
        <Button
          class="ai-conversation-composer__send"
          variant="primary"
          size="sm"
          :disabled="!canSend"
          :aria-label="t('ai.conversation.composer.send')"
          @click="submit"
        >
          <ArrowUp :size="14" aria-hidden="true" />
        </Button>
      </div>
    </div>

    <p class="ai-conversation-composer__hint">
      {{ t("ai.conversation.composer.hint") }}
    </p>
  </div>
</template>

<style scoped>
.ai-conversation-composer {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4) var(--space-4);
  background: var(--ws-sidebar);
  border-top: var(--border-1) solid var(--ws-border);
}

.ai-conversation-composer__context {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
  min-width: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-conversation-composer__context > svg {
  flex: none;
}

.ai-conversation-composer__context-label {
  flex: none;
}

.ai-conversation-composer__context-value {
  overflow: hidden;
  color: var(--ws-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-conversation-composer__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ai-conversation-composer__actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.ai-conversation-composer__send {
  flex: none;
  min-width: var(--size-control-sm);
}

.ai-conversation-composer__hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
}

.ai-conversation-composer.is-disabled {
  opacity: var(--opacity-disabled-control);
}

@media (prefers-reduced-motion: reduce) {
  .ai-conversation-composer__input {
    transition: none;
  }
}
</style>
