<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Feedback/Badge" node=13:34167 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

export type BadgeVariant =
  | "neutral"
  | "accent"
  | "count"
  | "success"
  | "warning"
  | "danger"
  | "info";

interface Props {
  /** 语义变体 —— Figma 定义 neutral/accent/count；success/warning/danger/info 为本地语义延伸 */
  variant?: BadgeVariant;
  /** 尺寸档 —— Figma：sm 16 / md 20 / lg 24（lg 文字 13px） */
  size?: "sm" | "md" | "lg";
  /** 描边变体（本地延伸）—— 透明底 + 同色描边。
   *  用于在形状上区分「已成立的正式事实」（实心语义底）与
   *  「AI 产出、尚待人工确认」（描边），与图标通道互为冗余。 */
  outline?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  variant: "neutral",
  size: "md",
  outline: false
});

const classes = computed(() =>
  cn("ui-badge", `ui-badge--${props.variant}`, `ui-badge--${props.size}`, {
    "ui-badge--outline": props.outline
  })
);
</script>

<template>
  <span :class="classes">
    <slot />
  </span>
</template>

<style scoped>
/* Figma DS/Feedback/Badge（13:34167）：radius 4 / 三档高 16/20/24 /
 * sm·md 文字 11px（micro）、lg 13px Regular / 内边距 0 4（sm）、0 8（md·lg） /
 * Neutral·Count：elevated 底 + border 描边 + 次文字；
 * Accent：#242238 底 + accent 描边 + accent 文字。 */
.ui-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-badge);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-caption);
  white-space: nowrap;
}

.ui-badge--sm {
  height: var(--size-badge-sm);
  padding: 0 var(--space-1);
}

.ui-badge--lg {
  height: var(--size-badge-lg);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.ui-badge--neutral,
.ui-badge--count {
  background: var(--surface-raised);
  color: var(--text-secondary);
}

.ui-badge--accent {
  background: var(--accent-surface);
  border-color: var(--accent);
  color: var(--accent);
}

/* 本地语义延伸（Figma 未定义；沿用反馈面色系，底为 elevated 中性面） */
.ui-badge--success,
.ui-badge--warning,
.ui-badge--danger,
.ui-badge--info {
  background: var(--surface-raised);
}

.ui-badge--success {
  color: var(--success-strong);
}

.ui-badge--warning {
  color: var(--warning-strong);
}

.ui-badge--danger {
  color: var(--danger-title);
}

.ui-badge--info {
  color: var(--info-strong);
}

/* 描边变体（本地延伸）：透明底 + 同色描边 */
.ui-badge--outline {
  background: transparent;
  border-color: currentcolor;
}
</style>
