<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Feedback/StatusIndicator" node=13:34203 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

/**
 * StatusIndicator —— Figma DS/Feedback/StatusIndicator（13:34203）。
 * 状态圆点（sm12/md16/lg20）+ 可选标签。
 * ⚠ Figma IMAGE-SVG 不返回 fills，圆点颜色未取到 —— 按语义色 token 映射实现
 *（neutral=border / info/success/warning/danger=语义色 / accent=AI 强调）。
 */

export type StatusIndicatorSize = "sm" | "md" | "lg";
export type StatusIndicatorTone =
  | "neutral"
  | "info"
  | "success"
  | "warning"
  | "danger"
  | "accent";

interface Props {
  status?: StatusIndicatorTone;
  size?: StatusIndicatorSize;
  label?: string;
}

const props = withDefaults(defineProps<Props>(), {
  status: "neutral",
  size: "md",
  label: ""
});

const hasLabel = computed(() => props.label.length > 0);
</script>

<template>
  <span
    :class="
      cn('ui-status', `ui-status--${size}`, `ui-status--${status}`, {
        'has-label': hasLabel
      })
    "
    role="status"
  >
    <span class="ui-status__dot" aria-hidden="true" />
    <span v-if="hasLabel" class="ui-status__label">{{ label }}</span>
  </span>
</template>

<style scoped>
.ui-status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.ui-status__dot {
  flex: none;
  border-radius: var(--radius-full);
  background-color: var(--border-strong);
}

.ui-status--sm .ui-status__dot {
  width: var(--size-status-dot-sm);
  height: var(--size-status-dot-sm);
}

.ui-status--md .ui-status__dot {
  width: var(--size-status-dot);
  height: var(--size-status-dot);
}

.ui-status--lg .ui-status__dot {
  width: var(--size-status-dot-lg);
  height: var(--size-status-dot-lg);
}

/* 语义映射（Figma 未取到点色，按语义色 token 推断） */
.ui-status--info .ui-status__dot {
  background-color: var(--ws-color-info);
}

.ui-status--success .ui-status__dot {
  background-color: var(--ws-color-success);
}

.ui-status--warning .ui-status__dot {
  background-color: var(--ws-color-warning);
}

.ui-status--danger .ui-status__dot {
  background-color: var(--ws-color-danger);
}

.ui-status--accent .ui-status__dot {
  background-color: var(--accent);
}

.ui-status__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}
</style>
