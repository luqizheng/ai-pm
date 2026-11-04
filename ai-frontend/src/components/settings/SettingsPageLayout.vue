<script setup lang="ts">
import type { Component } from "vue";

import Breadcrumb from "@/components/ui/breadcrumb/Breadcrumb.vue";
import type { BreadcrumbItem } from "@/components/ui/breadcrumb";

/**
 * SettingsPageLayout —— 设置页统一模板（Settings Framework §5/§7）。
 *
 * 结构 = 页头（面包屑 + 标题 + 描述 + 页级操作）+ 双栏主体
 * （设置侧栏 | 主内容）。右侧上下文面板为可选插槽（§5：不为空面板
 * 常驻预留空间），不传即不渲染。
 */
defineProps<{
  /** 面包屑层级 */
  breadcrumb: BreadcrumbItem[];
  /** 页标题（已翻译） */
  title: string;
  /** 页描述（已翻译；空省略） */
  description?: string;
  /** 侧栏图标（lucide；与条目 id 对应可选） */
  icons?: Record<string, Component>;
}>();
</script>

<template>
  <div class="settings-page">
    <header class="settings-page__header">
      <Breadcrumb :items="breadcrumb" />
      <div class="settings-page__heading">
        <div class="settings-page__text">
          <h1 class="settings-page__title">{{ title }}</h1>
          <p v-if="description" class="settings-page__desc">
            {{ description }}
          </p>
        </div>
        <div v-if="$slots.actions" class="settings-page__actions">
          <slot name="actions" />
        </div>
      </div>
    </header>

    <div class="settings-page__body">
      <aside class="settings-page__sidebar">
        <slot name="sidebar" />
      </aside>
      <main class="settings-page__main">
        <slot />
      </main>
      <aside v-if="$slots.aside" class="settings-page__aside">
        <slot name="aside" />
      </aside>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.settings-page__header {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-bottom: var(--space-4);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.settings-page__heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.settings-page__text {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.settings-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.settings-page__desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.settings-page__actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
}

.settings-page__body {
  display: grid;
  grid-template-columns: 13.5rem minmax(0, 1fr);
  gap: var(--space-8);
  align-items: start;
  padding-top: var(--space-5);
}

.settings-page__sidebar {
  position: sticky;
  top: var(--space-5);
}

.settings-page__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  min-width: 0;
}

.settings-page__aside {
  position: sticky;
  top: var(--space-5);
  width: var(--ws-context-width);
}

@media (max-width: 64rem) {
  .settings-page__body {
    grid-template-columns: minmax(0, 1fr);
  }

  .settings-page__sidebar {
    position: static;
  }

  .settings-page__aside {
    display: none;
  }
}
</style>
