<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Search/RecentSearches" node=33:85727 date=2026-11-04 -->
<script setup lang="ts">
import { ChevronRight, FileText } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import type { RecentSearch } from "./types";

/**
 * 最近搜索面板（Figma Global/Search/RecentSearches 33:85727）。
 *
 * surface 面板 + mono 11 群组标签 + ListItem 行（图标 / 标签 / Recent 提示 /
 * 右箭头）；点击行 = 把该查询重新填入搜索框（最近搜索是查询快捷入口）。
 * 空态由 SearchPalette 处理（EmptyState 提示块）。
 */
interface Props {
  items: RecentSearch[];
}

defineProps<Props>();

const emit = defineEmits<{ select: [item: RecentSearch] }>();

const { t } = useI18n();
</script>

<template>
  <section class="global-search-recent" :aria-label="t('global.search.recentLabel')">
    <p class="global-search-recent__label">
      {{ t("global.search.recentLabel") }}
    </p>
    <button
      v-for="item in items"
      :key="item.id"
      type="button"
      class="global-search-recent__row"
      @click="emit('select', item)"
    >
      <FileText :size="16" class="global-search-recent__icon" aria-hidden="true" />
      <span class="global-search-recent__text">{{ item.label }}</span>
      <span class="global-search-recent__hint">{{
        t("global.search.recentHint")
      }}</span>
      <ChevronRight :size="14" aria-hidden="true" />
    </button>
  </section>
</template>

<style scoped>
.global-search-recent {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-2);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

/* mono 群组标签（RECENT SEARCHES，10px 全大写） */
.global-search-recent__label {
  margin: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  letter-spacing: 0.08em;
}

.global-search-recent__row {
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

.global-search-recent__row:hover {
  background: var(--ws-hover-veil);
}

.global-search-recent__row:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.global-search-recent__icon {
  flex: none;
  color: var(--ws-text-secondary);
}

.global-search-recent__text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.global-search-recent__hint {
  flex: none;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.global-search-recent__row > svg:last-child {
  flex: none;
  color: var(--ws-text-secondary);
}
</style>
