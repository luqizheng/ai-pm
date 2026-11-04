<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/AI/CommandInput" node=13:34680 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { ArrowUp, LoaderCircle, Sparkles } from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * CommandInput —— Figma DS/AI/CommandInput（13:34680）。
 * AI 指令输入：sparkles 12px + 文本 + 右端发送（arrow-up 14px）；
 * Focus 2px accent；Loading 换 loader；下方可选上下文标签行（AI 可用上下文）。
 */

export type CommandInputSize = "sm" | "md" | "lg";

interface Props {
  modelValue?: string;
  placeholder?: string;
  size?: CommandInputSize;
  loading?: boolean;
  disabled?: boolean;
  sendDisabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  placeholder: "",
  size: "md",
  loading: false,
  disabled: false,
  sendDisabled: false
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
  send: [];
}>();

const canSend = computed(
  () => !props.disabled && !props.loading && !props.sendDisabled
);

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}

function onSend() {
  if (canSend.value) emit("send");
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    onSend();
  }
}
</script>

<template>
  <div
    :class="
      cn('ui-command-input', `ui-command-input--${size}`, {
        'is-disabled': disabled
      })
    "
  >
    <div class="ui-command-input__field">
      <LoaderCircle
        v-if="loading"
        class="ui-command-input__spinner"
        aria-hidden="true"
      />
      <Sparkles v-else class="ui-command-input__icon" aria-hidden="true" />
      <input
        class="ui-command-input__input"
        type="text"
        :value="modelValue"
        :placeholder="placeholder"
        :disabled="disabled"
        :aria-busy="loading ? 'true' : undefined"
        @input="onInput"
        @keydown="onKeydown"
      />
      <button
        type="button"
        class="ui-command-input__send"
        :disabled="!canSend"
        aria-label="Send"
        @click="onSend"
      >
        <ArrowUp aria-hidden="true" />
      </button>
    </div>
    <div v-if="$slots.context" class="ui-command-input__context">
      <slot name="context" />
    </div>
  </div>
</template>

<style scoped>
.ui-command-input {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
}

.ui-command-input.is-disabled {
  opacity: var(--opacity-disabled-field);
}

.ui-command-input__field {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background-color: var(--surface-sunken);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  transition: border-color var(--duration-fast) var(--ease-decelerate);
}

.ui-command-input--sm .ui-command-input__field {
  min-height: var(--size-control-sm);
}

.ui-command-input--md .ui-command-input__field {
  min-height: var(--size-nav-item);
}

.ui-command-input--lg .ui-command-input__field {
  min-height: var(--size-control-lg);
}

.ui-command-input__field:focus-within:not(.is-disabled) {
  border: var(--border-2) solid var(--accent);
}

.ui-command-input__icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  flex: none;
  color: var(--accent);
}

.ui-command-input__spinner {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
  animation: ui-command-input-spin var(--duration-spinner) linear infinite;
}

@keyframes ui-command-input-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ui-command-input__spinner {
    animation: none;
  }
}

.ui-command-input__input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ui-command-input__input::placeholder {
  color: var(--text-secondary);
}

.ui-command-input__input:focus {
  outline: none;
}

.ui-command-input__send {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-primary);
  cursor: pointer;
}

.ui-command-input__send:hover:not(:disabled) {
  background-color: var(--surface-raised);
}

.ui-command-input__send:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-command-input__send:disabled {
  color: var(--text-disabled);
  cursor: not-allowed;
}

.ui-command-input__send svg {
  width: var(--size-send-icon);
  height: var(--size-send-icon);
}

.ui-command-input__context {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
</style>
