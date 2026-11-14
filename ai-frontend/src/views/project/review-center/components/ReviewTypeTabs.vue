<script setup lang="ts">
/**
 * 类型页签（R03）：全部 / 发现 / 知识提案 / 交付验收。
 *
 * 纯展示件：文案与计数由调用方翻译后经 props 传入（D8）。
 */
export interface ReviewTabOption {
  key: string;
  label: string;
  count: number;
}

interface Props {
  tabs: ReviewTabOption[];
  modelValue: string;
}

defineProps<Props>();
defineEmits<{ "update:modelValue": [value: string] }>();
</script>

<template>
  <div class="review-tabs" role="tablist">
    <button
      v-for="tab in tabs"
      :key="tab.key"
      type="button"
      class="review-tabs__item"
      :class="{ 'is-active': tab.key === modelValue }"
      role="tab"
      :aria-selected="tab.key === modelValue"
      @click="$emit('update:modelValue', tab.key)"
    >
      {{ tab.label }}
      <span class="review-tabs__count">{{ tab.count }}</span>
    </button>
  </div>
</template>

<style scoped>
.review-tabs {
  display: flex;
  gap: var(--space-2, 8px);
  flex-wrap: wrap;
}

.review-tabs__item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition:
    background var(--duration-fast, 120ms) var(--ease-out, ease-out),
    color var(--duration-fast, 120ms) var(--ease-out, ease-out),
    border-color var(--duration-fast, 120ms) var(--ease-out, ease-out);
}

.review-tabs__item:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.review-tabs__item.is-active {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--text-primary);
}

.review-tabs__count {
  font-size: 12px;
  opacity: 0.85;
}
</style>
