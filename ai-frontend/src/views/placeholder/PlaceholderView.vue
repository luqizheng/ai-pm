<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import PageFrame from "@/components/shell/PageFrame.vue";

/**
 * 占位视图 —— 应用壳骨架阶段的默认业务页。
 *
 * = 统一页框 PageFrame（AGENTS.md §三.12）+ 「建设中」空态；
 * 渲染当前路由的面包屑 / 标题 / 描述与「建设中」空态；
 * 业务模块落地时按路由替换本组件。
 */

const { t } = useI18n();
const route = useRoute();

const title = computed(() =>
  route.meta.title
    ? t(route.meta.title as string)
    : t("shell.app.page.placeholderTitle")
);

const description = computed(() => t("shell.app.page.placeholderHint"));
</script>

<template>
  <PageFrame :title="title" :description="description">
    <div class="shell-placeholder__empty" role="status">
      <p class="shell-placeholder__empty-title">
        {{ t("shell.app.page.placeholderTitle") }}
      </p>
      <p class="shell-placeholder__empty-hint">
        {{ t("shell.app.page.placeholderHint") }}
      </p>
    </div>
  </PageFrame>
</template>

<style scoped>
.shell-placeholder__empty {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-8);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.shell-placeholder__empty-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
}

.shell-placeholder__empty-hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}
</style>
