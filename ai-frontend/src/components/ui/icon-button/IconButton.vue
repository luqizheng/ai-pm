<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Buttons/IconButton" node=13:32734 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { LoaderCircle } from "lucide-vue-next";
import type { Component } from "vue";
import { cn } from "@/lib/cn";

/**
 * IconButton —— Figma DS/Buttons/IconButton（13:32734）。
 * 方形命中区 · 16px 图标 · 变体 Secondary/Ghost/Danger × 尺寸 28/36/44。
 *
 * Figma 源异常备注（按正常口径实现）：
 *   - Secondary/Medium/Disabled 缺 0.38 不透明度（Small/Large 有）→ 统一 0.38；
 *   - Ghost/Large/Default 误带 danger 描边 → 忽略。
 */

export type IconButtonVariant = "secondary" | "ghost" | "danger";
export type IconButtonSize = "sm" | "md" | "lg";

interface Props {
  /** 无障碍名（图标按钮必填） */
  label: string;
  variant?: IconButtonVariant;
  size?: IconButtonSize;
  loading?: boolean;
  disabled?: boolean;
  type?: "button" | "submit" | "reset";
}

const props = withDefaults(defineProps<Props>(), {
  variant: "secondary",
  size: "md",
  loading: false,
  disabled: false,
  type: "button"
});

const isDisabled = computed(() => props.disabled || props.loading);

/** danger 变体默认图标由调用方经 slot 传入（如 Trash2），此处只承载 loading 图标 */
const slots = defineSlots<{ default?: () => Component }>();
void slots;
</script>

<template>
  <button
    :type="type"
    :disabled="isDisabled"
    :aria-busy="loading ? 'true' : undefined"
    :aria-label="label"
    :title="label"
    :class="
      cn(
        'ui-icon-button',
        `ui-icon-button--${variant}`,
        `ui-icon-button--${size}`
      )
    "
  >
    <LoaderCircle
      v-if="loading"
      class="ui-icon-button__spinner"
      aria-hidden="true"
    />
    <slot v-else />
  </button>
</template>

<style scoped>
.ui-icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-sm);
  padding: 0;
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-icon-button:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-icon-button:disabled {
  cursor: not-allowed;
}

/* Secondary：surface 底 + border 描边；hover 升 elevated / pressed 透明底 */
.ui-icon-button--secondary {
  background-color: var(--surface-base);
  border-color: var(--border-strong);
  color: var(--text-primary);
}

.ui-icon-button--secondary:hover:not(:disabled) {
  background-color: var(--surface-raised);
}

.ui-icon-button--secondary:active:not(:disabled) {
  background-color: transparent;
}

.ui-icon-button--secondary:disabled {
  opacity: var(--opacity-disabled-button);
}

/* Ghost：透明底 + 主文字色；hover elevated / pressed surface 底 */
.ui-icon-button--ghost {
  background-color: transparent;
  color: var(--text-primary);
}

.ui-icon-button--ghost:hover:not(:disabled) {
  background-color: var(--surface-raised);
}

.ui-icon-button--ghost:active:not(:disabled) {
  background-color: var(--surface-base);
}

.ui-icon-button--ghost:disabled {
  opacity: var(--opacity-disabled-button);
}

/* Danger：danger 反馈面底 + danger 描边；hover/pressed 透明底 */
.ui-icon-button--danger {
  background-color: var(--danger-surface);
  border-color: var(--danger-border);
  color: var(--danger-title);
}

.ui-icon-button--danger:hover:not(:disabled),
.ui-icon-button--danger:active:not(:disabled) {
  background-color: transparent;
}

.ui-icon-button--danger:focus-visible {
  outline-color: var(--danger-title);
}

.ui-icon-button--danger:disabled {
  opacity: var(--opacity-disabled-button);
}

/* 尺寸 —— 命中区 28/36/44，图标 16px 由 slot 内容自带 */
.ui-icon-button--sm {
  width: var(--size-control-sm);
  height: var(--size-control-sm);
}

.ui-icon-button--md {
  width: var(--size-icon-button);
  height: var(--size-icon-button);
}

.ui-icon-button--lg {
  width: var(--size-control-lg);
  height: var(--size-control-lg);
}

.ui-icon-button__spinner {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  animation: ui-icon-button-spin var(--duration-spinner) linear infinite;
}

@keyframes ui-icon-button-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ui-icon-button__spinner {
    animation: none;
  }
}
</style>
