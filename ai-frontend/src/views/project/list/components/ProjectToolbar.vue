<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { LayoutGrid, LayoutList, X } from "lucide-vue-next";
import SearchInput from "@/components/ui/search-input/SearchInput.vue";
import Select from "@/components/ui/select/Select.vue";
import type { SelectOption } from "@/components/ui/select/types";

/**
 * 项目工具栏（Projects List §5）：搜索 + 生命周期过滤 + 排序
 * + 「需要关注」过滤 chip + 列表/网格视图切换。
 * 全部受控（页根持有状态，指标条与工具栏联动）。
 */
interface Props {
  search: string;
  lifecycle: string;
  sortKey: string;
  viewMode: "list" | "grid";
  attentionOnly: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "update:search", value: string): void;
  (e: "update:lifecycle", value: string): void;
  (e: "update:sortKey", value: string): void;
  (e: "update:viewMode", value: "list" | "grid"): void;
  (e: "remove-attention"): void;
}>();

const { t } = useI18n();

const lifecycleOptions = computed<SelectOption[]>(() => [
  { value: "all", label: t("pm.projects.filterAll") },
  { value: "active", label: t("pm.projects.filterActive") },
  { value: "archived", label: t("pm.projects.filterArchived") }
]);

const sortOptions = computed<SelectOption[]>(() => [
  { value: "updated_desc", label: t("pm.projects.sortUpdated") },
  { value: "created_desc", label: t("pm.projects.sortCreated") },
  { value: "name_asc", label: t("pm.projects.sortName") }
]);
</script>

<template>
  <div class="proj-toolbar">
    <SearchInput
      :model-value="props.search"
      :placeholder="t('pm.projects.searchPlaceholder')"
      size="sm"
      class="proj-toolbar__search"
      :aria-label="t('pm.projects.searchPlaceholder')"
      @update:model-value="emit('update:search', $event)"
    />
    <Select
      :model-value="props.lifecycle"
      :options="lifecycleOptions"
      size="sm"
      :aria-label="t('pm.projects.filterLabel')"
      @update:model-value="emit('update:lifecycle', $event)"
    />
    <Select
      :model-value="props.sortKey"
      :options="sortOptions"
      size="sm"
      :aria-label="t('pm.projects.sortLabel')"
      @update:model-value="emit('update:sortKey', $event)"
    />
    <button
      v-if="props.attentionOnly"
      type="button"
      class="proj-toolbar__chip"
      :title="t('pm.projects.attentionChipRemove')"
      @click="emit('remove-attention')"
    >
      <span>{{ t("pm.projects.attentionChip") }}</span>
      <X aria-hidden="true" />
      <span class="ui-sr-only">{{ t("pm.projects.attentionChipRemove") }}</span>
    </button>

    <div
      class="proj-toolbar__view-toggle"
      role="group"
      :aria-label="t('pm.projects.viewLabel')"
    >
      <button
        type="button"
        class="proj-toolbar__view-btn"
        :class="{ 'proj-toolbar__view-btn--active': props.viewMode === 'list' }"
        :aria-pressed="props.viewMode === 'list'"
        :aria-label="t('pm.projects.viewList')"
        @click="emit('update:viewMode', 'list')"
      >
        <LayoutList aria-hidden="true" />
      </button>
      <button
        type="button"
        class="proj-toolbar__view-btn"
        :class="{ 'proj-toolbar__view-btn--active': props.viewMode === 'grid' }"
        :aria-pressed="props.viewMode === 'grid'"
        :aria-label="t('pm.projects.viewGrid')"
        @click="emit('update:viewMode', 'grid')"
      >
        <LayoutGrid aria-hidden="true" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.proj-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.proj-toolbar__search {
  width: var(--size-list-search-width);
}

.proj-toolbar__chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-full);
  background: var(--surface-raised);
  color: var(--warning-strong);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-standard);
}

.proj-toolbar__chip svg {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

.proj-toolbar__chip:hover {
  background: var(--ws-hover-veil);
}

.proj-toolbar__chip:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.proj-toolbar__view-toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin-left: auto;
}

.proj-toolbar__view-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard);
}

.proj-toolbar__view-btn svg {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
}

.proj-toolbar__view-btn:hover {
  background: var(--ws-hover-veil);
  color: var(--text-primary);
}

.proj-toolbar__view-btn--active {
  background: var(--ws-color-accent-surface);
  border-color: var(--accent);
  color: var(--accent);
}

.proj-toolbar__view-btn:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.ui-sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
  border: 0;
}
</style>
