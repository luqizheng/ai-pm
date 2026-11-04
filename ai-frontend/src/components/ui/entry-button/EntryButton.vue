<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/AI/EntryButton" node=13:34559 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { LoaderCircle, Sparkles } from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * EntryButton —— Figma DS/AI/EntryButton（13:34559）。
 * AI 入口按钮：sparkles 12px 图标 + 标签；36px 行 / radius 8；
 * 变体 default（侧栏底）/ accent（AI 建议走 accent 色）/ ghost（透明）。
 */

export type EntryButtonSize = "sm" | "md" | "lg";
export type EntryButtonVariant = "default" | "accent" | "ghost";

interface Props {
  label: string;
  variant?: EntryButtonVariant;
  size?: EntryButtonSize;
  loading?: boolean;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  variant: "default",
  size: "md",
  loading: false,
  disabled: false
});

const emit = defineEmits<{ click: [event: MouseEvent] }>();

const isDisabled = computed(() => props.disabled || props.loading);
</script>

<template>
  <button
    type="button"
    :class="
      cn(
        'ui-entry-button',
        `ui-entry-button--${variant}`,
        `ui-entry-button--${size}`,
        {
          'is-disabled': isDisabled
        }
      )
    "
    :aria-busy="loading ? 'true' : undefined"
    :disabled="isDisabled"
    @click="emit('click', $event)"
  >
    <LoaderCircle
      v-if="loading"
      class="ui-entry-button__spinner"
      aria-hidden="true"
    />
    <Sparkles v-else class="ui-entry-button__icon" aria-hidden="true" />
    <span class="ui-entry-button__label">{{ label }}</span>
  </button>
</template>

<style scoped>
.ui-entry-button {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate);
}

.ui-entry-button--sm {
  height: var(--size-control-sm);
}

.ui-entry-button--md {
  height: var(--size-nav-item);
}

.ui-entry-button--lg {
  height: var(--size-control-lg);
}

.ui-entry-button:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-entry-button.is-disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}

/* default：侧栏底 + 主文字 */
.ui-entry-button--default {
  background-color: var(--surface-sunken);
  color: var(--text-primary);
}

.ui-entry-button--default:hover:not(.is-disabled) {
  background-color: var(--surface-raised);
}

/* accent：AI 建议色（AGENTS.md：AI 建议必须 accent 可辨） */
.ui-entry-button--accent {
  background-color: var(--accent-surface);
  border-color: var(--accent);
  color: var(--accent);
}

.ui-entry-button--accent:hover:not(.is-disabled) {
  background-color: var(--ws-hover-veil);
}

/* ghost：透明底 */
.ui-entry-button--ghost {
  background-color: transparent;
  border-color: transparent;
  color: var(--text-primary);
}

.ui-entry-button--ghost:hover:not(.is-disabled) {
  background-color: var(--surface-raised);
}

.ui-entry-button__icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  flex: none;
  color: var(--accent);
}

.ui-entry-button__spinner {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  animation: ui-entry-button-spin var(--duration-spinner) linear infinite;
}

@keyframes ui-entry-button-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ui-entry-button__spinner {
    animation: none;
  }
}

.ui-entry-button__label {
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  white-space: nowrap;
}
</style>
