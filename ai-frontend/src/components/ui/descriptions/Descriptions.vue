<script setup lang="ts">
export interface DescriptionItem {
  label: string;
  value?: string | number | null;
}

interface Props {
  /** 键值条目 */
  items: DescriptionItem[];
  /** 栅格列数 */
  columns?: number;
}

const props = withDefaults(defineProps<Props>(), { columns: 2 });
</script>

<template>
  <dl class="ui-descriptions" :class="`ui-descriptions--cols-${props.columns}`">
    <div v-for="item in items" :key="item.label" class="ui-descriptions__item">
      <dt class="ui-descriptions__label">{{ item.label }}</dt>
      <dd class="ui-descriptions__value">{{ item.value ?? "-" }}</dd>
    </div>
  </dl>
</template>

<style scoped>
.ui-descriptions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4) var(--space-6);
  margin: 0;
}

/* columns 决定栅格列数；条目按列流排，不再整行占满 */
.ui-descriptions--cols-1 {
  grid-template-columns: minmax(0, 1fr);
}

.ui-descriptions--cols-3 {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.ui-descriptions__item {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.ui-descriptions__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-tertiary);
}

.ui-descriptions__value {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
  word-break: break-word;
}

@media (max-width: 640px) {
  .ui-descriptions {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
