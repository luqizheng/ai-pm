<script setup lang="ts">
/**
 * PA-06 页内右栏卡片（画布 Context panel：白卡 320px，屏专属内容）。
 * 标题 + 可选 accent 徽标 + 分节 slot（section 组件用 CollabContextSection）。
 */
defineProps<{
  title: string;
  badge?: string;
}>();
</script>

<template>
  <section class="ctx-panel">
    <h2 class="ctx-panel__title">{{ title }}</h2>
    <slot name="badge">
      <span v-if="badge" class="ctx-panel__badge">{{ badge }}</span>
    </slot>
    <slot />
  </section>
</template>

<style scoped>
.ctx-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.ctx-panel__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ctx-panel__badge {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  padding: 0 var(--space-2);
  height: 1.25rem;
  border: var(--border-1) solid var(--ws-accent);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--ws-accent) 12%, transparent);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-accent);
}
</style>
