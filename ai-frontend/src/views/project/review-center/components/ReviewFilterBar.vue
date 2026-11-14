<script setup lang="ts">
import Input from "@/components/ui/input/Input.vue";

/**
 * 队列筛选栏（V1.0 §4.2）：搜索 / 领域 / 排序。
 *
 * 纯展示件：选项与占位文案由调用方翻译后经 props 传入（D8）。
 * 紧急程度、责任人筛选无对应契约字段，不提供（V1.0 §4.2）。
 */
export interface ReviewSelectOption {
  value: string;
  label: string;
}

interface Props {
  modelSearch: string;
  searchPlaceholder: string;
  domains: ReviewSelectOption[];
  modelDomain: string;
  sortOptions: ReviewSelectOption[];
  modelSort: string;
}

defineProps<Props>();
defineEmits<{
  "update:modelSearch": [value: string];
  "update:modelDomain": [value: string];
  "update:modelSort": [value: string];
}>();
</script>

<template>
  <div class="review-filter">
    <Input
      class="review-filter__search"
      :model-value="modelSearch"
      type="search"
      size="sm"
      :placeholder="searchPlaceholder"
      @update:model-value="$emit('update:modelSearch', String($event))"
    />
    <select
      class="review-filter__select"
      :value="modelDomain"
      @change="
        $emit('update:modelDomain', ($event.target as HTMLSelectElement).value)
      "
    >
      <option
        v-for="option in domains"
        :key="option.value"
        :value="option.value"
      >
        {{ option.label }}
      </option>
    </select>
    <select
      class="review-filter__select"
      :value="modelSort"
      @change="
        $emit('update:modelSort', ($event.target as HTMLSelectElement).value)
      "
    >
      <option
        v-for="option in sortOptions"
        :key="option.value"
        :value="option.value"
      >
        {{ option.label }}
      </option>
    </select>
  </div>
</template>

<style scoped>
.review-filter {
  display: flex;
  gap: var(--space-2, 8px);
  align-items: center;
}

.review-filter__search {
  flex: 1;
  min-width: 0;
}

.review-filter__select {
  height: 32px;
  padding: 0 8px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
}

.review-filter__select:hover {
  background: var(--surface-hover-veil);
}
</style>
