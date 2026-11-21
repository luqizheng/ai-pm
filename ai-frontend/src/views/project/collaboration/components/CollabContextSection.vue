<script setup lang="ts">
/**
 * PA-06 右栏分节（画布小节模式：11px 大写眉题 + 内容）。
 */
defineProps<{
  label: string;
  /** 语义色：danger 用于红色警示（画布 D-019 Authorization boundary 类目） */
  tone?: "default" | "accent" | "danger";
}>();
</script>

<template>
  <div class="ctx-section" :class="tone ? `is-${tone}` : undefined">
    <p class="ctx-section__label">{{ label }}</p>
    <div class="ctx-section__body">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.ctx-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ctx-section__label {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.ctx-section.is-accent .ctx-section__body {
  color: var(--ws-accent);
}

.ctx-section.is-danger .ctx-section__body {
  color: var(--ws-color-danger-text);
}

.ctx-section__body {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
  white-space: pre-line;
}
</style>
