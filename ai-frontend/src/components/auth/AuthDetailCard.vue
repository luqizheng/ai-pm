<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2656 set="02 — Authentication / Invitation details" date=2026-11-04 -->
<script setup lang="ts">
/**
 * 认证页信息卡 —— 画布底 + 1px 描边 + radius 8 的内嵌信息块。
 *
 * Figma 三处共用同一外壳：邀请详情（padding 16 / 间距 16）、
 * 访问详情（padding 16 / 间距 12）、上次登录账号（padding 12 / 间距 4）。
 */
interface Props {
  /** 内边距档 —— Figma：sm 12 / md 16 */
  padding?: "sm" | "md";
  /** 纵向间距档 —— Figma：sm 4 / md 12 */
  gap?: "sm" | "md";
}

withDefaults(defineProps<Props>(), {
  padding: "md",
  gap: "md"
});
</script>

<template>
  <div class="auth-detail-card" :data-padding="padding" :data-gap="gap">
    <slot />
  </div>
</template>

<style scoped>
.auth-detail-card {
  display: flex;
  flex-direction: column;
  width: 100%;
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.auth-detail-card[data-padding="sm"] {
  padding: var(--space-3);
}

.auth-detail-card[data-padding="md"] {
  padding: var(--space-4);
}

.auth-detail-card[data-gap="sm"] {
  gap: var(--space-1);
}

.auth-detail-card[data-gap="md"] {
  gap: var(--space-3);
}
</style>
