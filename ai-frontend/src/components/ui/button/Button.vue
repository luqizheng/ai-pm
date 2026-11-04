<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Buttons/Button" node=13:32697 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";
import {
  BUTTON_SIZES,
  BUTTON_VARIANTS,
  type ButtonSize,
  type ButtonVariant
} from "./button";

interface Props {
  /** 视觉变体 —— Figma 组件库五变体（primary/secondary/tertiary/ghost/danger） */
  variant?: ButtonVariant;
  /** 尺寸系 —— Figma：sm 28 / md 36 / lg 44 / 图标方钮 */
  size?: ButtonSize;
  /** 异步进行中：禁用按钮并标记 aria-busy（内嵌 loader 图标） */
  loading?: boolean;
  disabled?: boolean;
  type?: "button" | "submit" | "reset";
}

const props = withDefaults(defineProps<Props>(), {
  variant: "primary",
  size: "md",
  loading: false,
  disabled: false,
  type: "button"
});

const isDisabled = computed(() => props.disabled || props.loading);
</script>

<template>
  <button
    :type="type"
    :disabled="isDisabled"
    :aria-busy="loading ? 'true' : undefined"
    :class="
      cn('ui-button', `ui-button--${variant}`, `ui-button--${size}`, {
        'is-loading': loading
      })
    "
  >
    <span v-if="loading" class="ui-button__spinner" aria-hidden="true" />
    <slot />
  </button>
</template>

<style scoped>
/* Figma 组件库 DS/Buttons/Button（13:32697）：
 * 13px Medium / padding 0 12 / gap 8 / radius 8 / sm28 md36 lg44。
 * 态色全部经 token 换轨取 Figma 值（--accent-hover/--accent-pressed/
 * --danger-surface/--danger-border/--danger-title/--text-disabled）。 */
.ui-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  border: var(--border-1) solid transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-body);
  border-radius: var(--radius-sm);
  padding: 0 var(--space-3);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-button:disabled {
  cursor: not-allowed;
}

/* 焦点 —— Figma：2px 描边（Primary 用亮色环，其余用 accent 环） */
.ui-button:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

/* Primary：accent 底 + canvas 文字；hover #9B94FF / pressed #7971DE */
.ui-button--primary {
  background-color: var(--accent);
  color: var(--accent-contrast-text);
}

.ui-button--primary:hover:not(:disabled) {
  background-color: var(--accent-hover);
}

.ui-button--primary:active:not(:disabled) {
  background-color: var(--accent-pressed);
}

.ui-button--primary:focus-visible {
  outline-color: var(--text-primary);
}

/* Primary disabled：elevated 底 + 禁用文字（Figma Medium 档口径） */
.ui-button--primary:disabled {
  background-color: var(--surface-raised);
  color: var(--text-disabled);
}

/* Secondary：surface 底 + border 描边；hover 升 elevated / pressed 透明底 */
.ui-button--secondary {
  background-color: var(--surface-base);
  border-color: var(--border-strong);
  color: var(--text-primary);
}

.ui-button--secondary:hover:not(:disabled) {
  background-color: var(--surface-raised);
}

.ui-button--secondary:active:not(:disabled) {
  background-color: transparent;
}

.ui-button--secondary:disabled {
  opacity: var(--opacity-disabled-button);
}

/* Tertiary：透明底 + accent 文字；hover elevated / pressed surface 底 */
.ui-button--tertiary {
  background-color: transparent;
  color: var(--accent);
}

.ui-button--tertiary:hover:not(:disabled) {
  background-color: var(--surface-raised);
}

.ui-button--tertiary:active:not(:disabled) {
  background-color: var(--surface-base);
}

.ui-button--tertiary:disabled {
  opacity: var(--opacity-disabled-button);
}

/* Ghost：透明底 + 主文字；hover elevated / pressed surface 底 */
.ui-button--ghost {
  background-color: transparent;
  color: var(--text-primary);
}

.ui-button--ghost:hover:not(:disabled) {
  background-color: var(--surface-raised);
}

.ui-button--ghost:active:not(:disabled) {
  background-color: var(--surface-base);
}

.ui-button--ghost:disabled {
  opacity: var(--opacity-disabled-button);
}

/* Danger：danger-surface 底 + danger 描边/文字；hover/pressed 透明底 */
.ui-button--danger {
  background-color: var(--danger-surface);
  border-color: var(--danger-border);
  color: var(--danger-title);
}

.ui-button--danger:hover:not(:disabled),
.ui-button--danger:active:not(:disabled) {
  background-color: transparent;
}

.ui-button--danger:focus-visible {
  outline-color: var(--danger-title);
}

.ui-button--danger:disabled {
  opacity: var(--opacity-disabled-button);
}

/* 尺寸系 —— 字号三档统一 13px（Figma） */
.ui-button--sm {
  height: var(--size-control-sm);
}

.ui-button--md {
  height: var(--size-control-md);
}

.ui-button--lg {
  height: var(--size-control-lg);
}

.ui-button--icon {
  width: var(--size-icon-button);
  height: var(--size-icon-button);
  padding: 0;
}

/* 加载指示 —— Figma 内嵌图标 16px */
.ui-button__spinner {
  flex: none;
  width: var(--size-spinner);
  height: var(--size-spinner);
  border: var(--border-2) solid currentcolor;
  border-right-color: transparent;
  border-radius: var(--radius-full);
  animation: ui-button-spin var(--duration-spinner) linear infinite;
}

@keyframes ui-button-spin {
  to {
    transform: rotate(360deg);
  }
}

/* 减少动效 —— 仅停 spinner，颜色过渡保留 */
@media (prefers-reduced-motion: reduce) {
  .ui-button__spinner {
    animation: none;
  }
}
</style>
