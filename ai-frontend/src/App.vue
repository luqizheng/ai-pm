<script setup lang="ts">
/**
 * 基础占位页 —— 仅用于让工程可启动。
 * 重建 Frontend Foundation（认证 / 全局壳 / 导航 / 命令面板 …）时整体替换本文件。
 */
</script>

<template>
  <main class="foundation-placeholder">
    <h1>AI R&amp;D Workspace</h1>
    <p>
      Frontend foundation scaffold — app shell &amp; pages pending rebuild per
      AGENTS.md.
    </p>
  </main>
</template>

<style scoped>
.foundation-placeholder {
  display: grid;
  place-content: center;
  gap: calc(var(--ws-space-unit) * 3);
  min-height: 100dvh;
  background: var(--ws-canvas);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
}

.foundation-placeholder h1 {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: 600;
}

.foundation-placeholder p {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
}
</style>
