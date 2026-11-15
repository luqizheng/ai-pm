<script setup lang="ts">
import SearchForm from "@/components/ui/search-form/SearchForm.vue";
import SearchInput from "@/components/ui/search-input/SearchInput.vue";
import Select from "@/components/ui/select/Select.vue";

/**
 * 队列筛选栏（V1.0 §4.2）：搜索 / 领域 / 排序。
 *
 * 容器 = DS 组件 SearchForm（治 ui-select 触发器 width:100% 竖排的统一口径），
 * 搜索 = DS 组件 SearchInput，下拉 = DS 组件 Select（Figma DS/Forms/Select）。
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
  /** 未选 = undefined（reka 以空串为「已选未命中」，会吞掉 placeholder 渲染） */
  modelDomain: string | undefined;
  sortOptions: ReviewSelectOption[];
  modelSort: string;
  /** 领域下拉的「全部」占位文案（reka 以空串表达未选值，全部态走 placeholder） */
  domainAllLabel: string;
  /** 下拉的无障碍名（调用方注入 i18n） */
  domainAriaLabel?: string;
  sortAriaLabel?: string;
}

withDefaults(defineProps<Props>(), {
  domainAriaLabel: undefined,
  sortAriaLabel: undefined
});
defineEmits<{
  "update:modelSearch": [value: string];
  "update:modelDomain": [value: string];
  "update:modelSort": [value: string];
}>();
</script>

<template>
  <SearchForm>
    <template #search>
      <SearchInput
        :model-value="modelSearch"
        :placeholder="searchPlaceholder"
        :shortcut="''"
        @update:model-value="$emit('update:modelSearch', String($event))"
      />
    </template>
    <Select
      :model-value="modelDomain"
      :options="domains"
      :placeholder="domainAllLabel"
      clearable
      :aria-label="domainAriaLabel"
      @update:model-value="$emit('update:modelDomain', String($event))"
    />
    <Select
      :model-value="modelSort"
      :options="sortOptions"
      :aria-label="sortAriaLabel"
      @update:model-value="$emit('update:modelSort', String($event))"
    />
  </SearchForm>
</template>
