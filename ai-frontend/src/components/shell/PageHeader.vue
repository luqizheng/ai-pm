<script setup lang="ts">
import { useI18n } from "vue-i18n";

import Breadcrumb from "@/components/ui/breadcrumb/Breadcrumb.vue";

/**
 * 页面标题区（Application Shell §2C）。
 *
 * 顺序：面包屑 → 标题 + 描述 → 页面级操作（插槽）。
 */

interface Props {
  /** 面包屑条目 */
  breadcrumbs?: Array<{ label: string; to?: string }>;
  /** 页面标题 */
  title: string;
  /** 页面描述 */
  description?: string;
}

const props = withDefaults(defineProps<Props>(), {
  breadcrumbs: () => [],
  description: undefined
});

const { t } = useI18n();
</script>

<template>
  <header class="shell-page-header">
    <Breadcrumb
      v-if="props.breadcrumbs.length"
      class="shell-page-header__breadcrumb"
      :items="props.breadcrumbs"
    />

    <div class="shell-page-header__row">
      <div class="shell-page-header__identity">
        <h1 class="shell-page-header__title">{{ props.title }}</h1>
        <p v-if="props.description" class="shell-page-header__description">
          {{ props.description }}
        </p>
      </div>
      <div v-if="$slots.actions" class="shell-page-header__actions">
        <slot name="actions" />
      </div>
    </div>
  </header>
</template>

<style scoped>
.shell-page-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.shell-page-header__row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.shell-page-header__identity {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.shell-page-header__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
}

.shell-page-header__description {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

.shell-page-header__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}
</style>
