<script setup lang="ts">
import { useI18n } from "vue-i18n";

import DesignLayout from "../../DesignLayout.vue";

/**
 * AI 交互组件库目录页的公共页框：
 * 标题 / 副标题 / 页脚说明统一，正文由各页以 DesignFrame 组织。
 */
withDefaults(
  defineProps<{
    titleKey: string;
    subtitleKey: string;
    /** 页脚补充说明（可选 i18n key） */
    noteKey?: string;
  }>(),
  { noteKey: "" }
);

const { t } = useI18n();
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="catalog-page__title">{{ t(titleKey) }}</h1>
      <p class="catalog-page__subtitle">{{ t(subtitleKey) }}</p>
    </template>

    <slot />

    <p v-if="noteKey" class="catalog-page__note">{{ t(noteKey) }}</p>
  </DesignLayout>
</template>

<style scoped>
.catalog-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.catalog-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.catalog-page__note {
  margin: var(--space-6) 0 0;
  padding-top: var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  line-height: var(--line-height-micro);
}
</style>
