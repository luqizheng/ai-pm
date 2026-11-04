<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2500 set="02 — Authentication / Authentication panel" date=2026-11-04 -->
<script setup lang="ts">
/**
 * 认证面板 —— Figma 九帧共用的内容面板。
 *
 * 400px 宽 / 表面底 + 1px 描边 / radius 8 / 内边距 24 / 纵向间距 24。
 * 面板内部纵向排布由调用方用「分组」实现（标题组 / 表单组 / 提供方组），
 * 本组件只负责面板外壳。
 */
</script>

<template>
  <section class="auth-panel">
    <slot />
  </section>
</template>

<style scoped>
.auth-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  width: var(--ws-auth-panel-width);
  max-width: 100%;
  padding: var(--space-6);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}
</style>
