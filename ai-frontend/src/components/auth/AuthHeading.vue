<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2501 set="02 — Authentication / Panel heading" date=2026-11-04 -->
<script setup lang="ts">
import type { Component } from "vue";

/**
 * 面板标题组 —— 用途图标 + 主标题 + 副标题。
 *
 * Figma：用途图标 32×32（accent 着色面底 + radius 8 + 图标 16 accent 色）；
 * 主标题 22px SemiBold；副标题 13px Regular 次文字。
 */
interface Props {
  /** 用途图标组件（lucide） */
  icon?: Component;
  /** 主标题 */
  title: string;
  /** 副标题 */
  subtitle?: string;
  /** 用途图标持续旋转（Figma 认证进行中页的 loader-circle） */
  spin?: boolean;
}

withDefaults(defineProps<Props>(), {
  icon: undefined,
  subtitle: undefined,
  spin: false
});
</script>

<template>
  <div class="auth-heading">
    <span v-if="icon" class="auth-heading__indicator">
      <component
        :is="icon"
        class="auth-heading__icon"
        :class="{ 'auth-heading__icon--spin': spin }"
        aria-hidden="true"
      />
    </span>
    <h1 class="auth-heading__title">{{ title }}</h1>
    <p v-if="subtitle" class="auth-heading__subtitle">{{ subtitle }}</p>
  </div>
</template>

<style scoped>
.auth-heading {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.auth-heading__indicator {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: var(--ws-auth-indicator-size);
  height: var(--ws-auth-indicator-size);
  margin-bottom: var(--space-2);
  border-radius: var(--radius-sm);
  background: var(--ws-color-accent-surface);
  color: var(--ws-accent);
}

.auth-heading__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

/* 旋转图标 —— 认证进行中页（reduced-motion 下停转） */
.auth-heading__icon--spin {
  animation: auth-heading-spin var(--duration-spinner) linear infinite;
}

@keyframes auth-heading-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .auth-heading__icon--spin {
    animation: none;
  }
}

.auth-heading__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-page);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h1);
  color: var(--ws-text-primary);
}

.auth-heading__subtitle {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--ws-text-secondary);
}
</style>
