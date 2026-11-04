<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2529 set="02 — Authentication / Provider divider" date=2026-11-04 -->
<script setup lang="ts">
/**
 * 身份提供方分隔线 —— 「或使用以下方式继续」两侧的 1px 细线。
 *
 * Figma：左右细线各 1px（--ws-border），中间 11px 次文字文案，间距 12px。
 */
interface Props {
  /** 分隔文案 */
  label: string;
}

defineProps<Props>();
</script>

<template>
  <div class="auth-provider-divider">
    <span class="auth-provider-divider__rule" aria-hidden="true" />
    <span class="auth-provider-divider__label">{{ label }}</span>
    <span class="auth-provider-divider__rule" aria-hidden="true" />
  </div>
</template>

<style scoped>
.auth-provider-divider {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
}

.auth-provider-divider__rule {
  flex: 1;
  height: var(--border-1);
  background: var(--ws-border);
}

.auth-provider-divider__label {
  flex: none;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}
</style>
