<script setup lang="ts">
import Tabs from "@/components/ui/tabs/Tabs.vue";

/**
 * 类型页签（R03）：全部 / 发现 / 知识提案 / 交付验收。
 *
 * 视觉与交互 = DS 组件 Tabs（Figma DS/Navigation/Tab，13:33477 下划线口径）。
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

const props = defineProps<Props>();
defineEmits<{ "update:modelValue": [value: string] }>();

const tabItems = () =>
  props.tabs.map(tab => ({ value: tab.key, label: tab.label }));

const countOf = (value: string): number =>
  props.tabs.find(tab => tab.key === value)?.count ?? 0;
</script>

<template>
  <div class="review-tabs">
    <Tabs
      :model-value="modelValue"
      :tabs="tabItems()"
      @update:model-value="$emit('update:modelValue', $event)"
    >
      <template #tab="{ tab }">
        {{ tab.label }}
        <span class="review-tabs__count">{{ countOf(tab.value) }}</span>
      </template>
    </Tabs>
  </div>
</template>

<style scoped>
/* 队列列较窄（V1.1 §2.1 474px，AI Chat 开启时更窄）：页签行允许换行，
 * 避免DS TabsList 的 nowrap 在窄容器里被裁切 */
.review-tabs :deep(.ui-tabs__list) {
  flex-wrap: wrap;
}

/* 计数徽数：DS Tab 内 trailing 小字，tabular-nums 对齐 */
.review-tabs__count {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-variant-numeric: tabular-nums;
  color: var(--text-secondary);
}
</style>
