<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/AI/StatusIndicator" node=13:34798 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

/**
 * AI StatusIndicator —— Figma DS/AI/StatusIndicator（13:34798）。
 * AI 状态点（sm8/md10/lg12）+ 可选标签。
 * ⚠ Figma IMAGE-SVG 不返回 fills，点色未取到 —— 按状态语义映射 token 推断：
 * idle=次文字 / thinking=accent / waiting=warning / done=success / failed=danger。
 */

export type AiStatusSize = "sm" | "md" | "lg";
export type AiStatus = "idle" | "thinking" | "waiting" | "done" | "failed";

interface Props {
  status?: AiStatus;
  size?: AiStatusSize;
  label?: string;
}

const props = withDefaults(defineProps<Props>(), {
  status: "idle",
  size: "md",
  label: ""
});

const hasLabel = computed(() => props.label.length > 0);
</script>

<template>
  <span
    :class="
      cn('ui-ai-status', `ui-ai-status--${size}`, `is-${status}`, {
        'has-label': hasLabel
      })
    "
    role="status"
  >
    <span class="ui-ai-status__dot" aria-hidden="true" />
    <span v-if="hasLabel" class="ui-ai-status__label">{{ label }}</span>
  </span>
</template>

<style scoped>
.ui-ai-status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.ui-ai-status__dot {
  flex: none;
  border-radius: var(--radius-full);
  background-color: var(--text-secondary);
}

.ui-ai-status--sm .ui-ai-status__dot {
  width: var(--size-ai-dot-sm);
  height: var(--size-ai-dot-sm);
}

.ui-ai-status--md .ui-ai-status__dot {
  width: var(--size-ai-dot);
  height: var(--size-ai-dot);
}

.ui-ai-status--lg .ui-ai-status__dot {
  width: var(--size-ai-dot-lg);
  height: var(--size-ai-dot-lg);
}

/* 状态语义映射（Figma 未取到点色，按语义推断） */
.ui-ai-status.is-thinking .ui-ai-status__dot {
  background-color: var(--accent);
}

.ui-ai-status.is-waiting .ui-ai-status__dot {
  background-color: var(--ws-color-warning);
}

.ui-ai-status.is-done .ui-ai-status__dot {
  background-color: var(--ws-color-success);
}

.ui-ai-status.is-failed .ui-ai-status__dot {
  background-color: var(--ws-color-danger);
}

.ui-ai-status__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}
</style>
