<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2524 set="02 — Authentication / Text action" date=2026-11-04 -->
<script setup lang="ts">
import type { Component } from "vue";

/**
 * 文字动作 —— 认证页里的次级动作（忘记密码 / 返回登录 / 换个账号等）。
 *
 * Figma 口径：accent 文字色、12px Medium、可带 14px 前置图标。
 * 给 `to` 渲染 RouterLink（可键盘聚焦、可中键新开），否则渲染按钮。
 */
interface Props {
  /** 动作文案 */
  label: string;
  /** 前置图标（lucide） */
  icon?: Component;
  /** 路由目标；缺省时按按钮渲染并抛出 click */
  to?: string;
}

withDefaults(defineProps<Props>(), {
  icon: undefined,
  to: undefined
});

const emit = defineEmits<{ click: [] }>();
</script>

<template>
  <RouterLink v-if="to" class="auth-text-action" :to="to">
    <component
      :is="icon"
      v-if="icon"
      class="auth-text-action__icon"
      aria-hidden="true"
    />
    <span>{{ label }}</span>
  </RouterLink>
  <button v-else type="button" class="auth-text-action" @click="emit('click')">
    <component
      :is="icon"
      v-if="icon"
      class="auth-text-action__icon"
      aria-hidden="true"
    />
    <span>{{ label }}</span>
  </button>
</template>

<style scoped>
.auth-text-action {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--ws-accent);
  text-decoration: none;
  cursor: pointer;
}

.auth-text-action:hover {
  color: var(--ws-accent-hover);
}

.auth-text-action:active {
  color: var(--ws-accent-pressed);
}

.auth-text-action:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: var(--space-1);
  border-radius: var(--radius-xs);
}

.auth-text-action__icon {
  width: var(--space-3);
  height: var(--space-3);
}
</style>
