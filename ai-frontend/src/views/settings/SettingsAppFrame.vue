<script setup lang="ts">
import type { Component } from "vue";

import SettingsPageLayout from "@/components/settings/SettingsPageLayout.vue";
import SettingsSidebar from "@/components/settings/SettingsSidebar.vue";
import type { SettingsNavEntry } from "@/components/settings";
import type { BreadcrumbItem } from "@/components/ui/breadcrumb";

/**
 * 应用设置页框 —— 设计目录 SettingsFrame 的 app 版（无 DesignLayout）。
 *
 * 组合 SettingsPageLayout + SettingsSidebar；页面在 WorkspaceShell
 * 应用壳内渲染（路由无 meta.layout，同 /notifications 先例）。
 */
defineProps<{
  /** 面包屑层级 */
  breadcrumb: BreadcrumbItem[];
  /** 页标题（已翻译） */
  title: string;
  /** 页描述（已翻译；空省略） */
  description?: string;
  /** 侧栏域标题（已翻译） */
  scopeLabel: string;
  /** 侧栏条目（应用路径） */
  entries: SettingsNavEntry[];
  /** 侧栏图标（与条目 id 对应可选） */
  icons?: Record<string, Component>;
}>();
</script>

<template>
  <SettingsPageLayout
    :breadcrumb="breadcrumb"
    :title="title"
    :description="description"
    :icons="icons"
  >
    <template #sidebar>
      <SettingsSidebar
        :scope-label="scopeLabel"
        :entries="entries"
        :icons="icons"
      />
    </template>
    <slot />
  </SettingsPageLayout>
</template>
