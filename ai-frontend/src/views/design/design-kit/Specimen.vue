<script setup lang="ts">
/**
 * Specimen —— 设计目录样板容器：mono 小标签 + 预览盒（surface 底 + 描边）。
 * 标注文字为 Figma 样板板原文（设计参考文本，英文）。
 */
defineProps<{
  /** 样板标注（如 State=Default） */
  label: string;
  /** 预览盒宽度（token 名或任意 CSS 宽度值）；默认自适应 */
  width?: string;
}>();
</script>

<template>
  <figure class="design-specimen">
    <figcaption class="design-specimen__label">{{ label }}</figcaption>
    <div
      class="design-specimen__preview"
      :style="width ? { width } : undefined"
    >
      <slot />
    </div>
  </figure>
</template>

<style scoped>
.design-specimen {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.design-specimen__label {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-specimen__preview {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  min-width: 200px;
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}
</style>
