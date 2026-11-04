<script setup lang="ts">
import { computed } from "vue";
import type { Component } from "vue";
import { useI18n } from "vue-i18n";

import {
  SettingsPageLayout,
  SettingsSidebar,
  SettingsPermissionNotice,
  PERSONA_LABEL_KEYS
} from "@/components/settings";
import type { BreadcrumbItem } from "@/components/ui/breadcrumb";
import type { SettingsNavEntry, SettingsPersona } from "@/components/settings";

import DesignLayout from "../DesignLayout.vue";

/**
 * SettingsFrame —— 设计目录内设置页的公共页框。
 *
 * 组合 SettingsPageLayout + SettingsSidebar；条目过滤与 denied 态
 * （§6.9 权限不足）由各域视图按 fixture 计算后传入。
 */
const props = defineProps<{
  breadcrumb: BreadcrumbItem[];
  title: string;
  description?: string;
  /** 侧栏域标题（已翻译） */
  scopeLabel: string;
  entries: SettingsNavEntry[];
  icons?: Record<string, Component>;
  /** 整域无授权：条目置灰 + 顶部 denied 横幅 */
  denied?: boolean;
  /** denied / readonly 横幅插值的演示角色 */
  persona?: SettingsPersona;
}>();

const { t } = useI18n();

const personaLabel = computed(() =>
  props.persona ? t(PERSONA_LABEL_KEYS[props.persona]) : undefined
);
</script>

<template>
  <DesignLayout>
    <SettingsPageLayout
      :breadcrumb="breadcrumb"
      :title="title"
      :description="description"
      :icons="icons"
    >
      <template #actions>
        <slot name="actions" />
      </template>
      <template #sidebar>
        <SettingsPermissionNotice
          v-if="denied"
          access="denied"
          :role="personaLabel"
        />
        <SettingsSidebar
          :scope-label="scopeLabel"
          :entries="entries"
          :icons="icons"
          :denied="denied"
        />
      </template>
      <slot />
    </SettingsPageLayout>
  </DesignLayout>
</template>
