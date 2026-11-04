<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { useI18n } from "vue-i18n";

import { useShellStore } from "@/stores/shell";
import DesignLayout from "../DesignLayout.vue";
import DesignShellFrame from "../design-kit/DesignShellFrame.vue";

/**
 * /design/navigation/global —— Figma 板 01（展开）/ 02（折叠）。
 * 真实应用壳渲染在 1440×900 画框；折叠按钮在壳内（SideNav 底部），
 * 经全局 store 切换 —— 画框 grid 随 data-sidebar 收列宽（复刻两板差异）。
 */
const { t } = useI18n();
const shell = useShellStore();

onMounted(() => {
  // 进入本板默认展开态（板 01）
  shell.setSidebarCollapsed(false);
});

onBeforeUnmount(() => {
  // 演示态不残留：离开时回到展开并关闭右栏开关按壳默认
  shell.setSidebarCollapsed(false);
});
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.navigation.global.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.navigation.global.subtitle") }}
      </p>
    </template>

    <DesignShellFrame
      label="Board 01 — Global Navigation / Expanded (7:14792)"
    />

    <p class="design-page__hint">
      {{ t("design.notes.board02") }}
    </p>
  </DesignLayout>
</template>

<style scoped>
.design-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-page__hint {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}
</style>
