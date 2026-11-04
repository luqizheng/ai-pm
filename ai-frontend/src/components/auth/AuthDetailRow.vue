<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2663 set="02 — Authentication / Invitation attribute" date=2026-11-04 -->
<script setup lang="ts">
/**
 * 信息行 —— 标签（11px 次文字）+ 取值（12px 主文字）两行式。
 *
 * Figma：邀请详情与访问详情共用（标签 11 Regular 次文字 / 取值 12 Regular 主文字）。
 */
interface Props {
  /** 标签文案 */
  label: string;
  /** 取值文案 */
  value: string;
}

defineProps<Props>();
</script>

<template>
  <div class="auth-detail-row">
    <span class="auth-detail-row__label">{{ label }}</span>
    <span class="auth-detail-row__value">{{ value }}</span>
  </div>
</template>

<style scoped>
.auth-detail-row {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.auth-detail-row__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

.auth-detail-row__value {
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-primary);
}
</style>
