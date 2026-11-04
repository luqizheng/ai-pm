<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Search/ResultRow" node=33:87782 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import {
  BookOpen,
  ChevronRight,
  FileText,
  Folder,
  GitBranch,
  ListTodo,
  Sparkles
} from "lucide-vue-next";
import type { Component } from "vue";

import type { SearchResultItem, SearchResultState } from "./types";

/**
 * 搜索结果行（Figma Global/Search/ResultRow 33:87782）。
 *
 * 六类对象（Projects / Repositories / Requirements / Tasks /
 * AI Execution Runs / Knowledge Documents）× 三态：
 *   - Default：surface 底 + 1px 描边
 *   - Selected：accent 着色面（#242238 / 浅色 #EEEBFF）
 *   - Keyboard focus：2px accent 描边（与 Selected 互相独立，可同时各占一行）
 * 行下方可带 11px 元数据行（meta 不参与分类，Figma 口径）。
 */
interface Props {
  item: SearchResultItem;
  state?: SearchResultState;
}

const props = withDefaults(defineProps<Props>(), {
  state: "default"
});

const emit = defineEmits<{ select: [item: SearchResultItem] }>();

const ICONS: Record<SearchResultItem["objectType"], Component> = {
  project: Folder,
  repository: GitBranch,
  requirement: FileText,
  task: ListTodo,
  aiRun: Sparkles,
  document: BookOpen
};

const icon = computed(() => ICONS[props.item.objectType]);
</script>

<template>
  <div class="global-search-result">
    <button
      type="button"
      class="global-search-result__row"
      :class="`is-${props.state}`"
      role="option"
      :aria-selected="props.state !== 'default'"
      @click="emit('select', props.item)"
    >
      <component
        :is="icon"
        :size="16"
        class="global-search-result__icon"
        aria-hidden="true"
      />
      <span class="global-search-result__label">{{ props.item.label }}</span>
      <span v-if="props.item.description" class="global-search-result__desc">{{
        props.item.description
      }}</span>
      <ChevronRight
        :size="14"
        class="global-search-result__chevron"
        aria-hidden="true"
      />
    </button>
    <p v-if="props.item.meta" class="global-search-result__meta">
      {{ props.item.meta }}
    </p>
  </div>
</template>

<style scoped>
.global-search-result {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.global-search-result__row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-row-height);
  padding: 0 var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  text-align: left;
  cursor: pointer;
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.global-search-result__row:hover {
  background: var(--ws-hover-veil);
}

.global-search-result__row:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* Selected：accent 着色面 */
.global-search-result__row.is-selected {
  background: var(--ws-color-accent-surface);
}

/* Keyboard focus：2px accent 描边（1px 描边变色 + 内嵌 1px 补足） */
.global-search-result__row.is-keyboardFocus {
  border-color: var(--ws-accent);
  box-shadow: inset 0 0 0 1px var(--ws-accent);
}

.global-search-result__icon {
  flex: none;
  color: var(--ws-text-secondary);
}

.global-search-result__label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.global-search-result__desc {
  flex: none;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.global-search-result__chevron {
  flex: none;
  color: var(--ws-text-secondary);
}

.global-search-result__meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}
</style>
