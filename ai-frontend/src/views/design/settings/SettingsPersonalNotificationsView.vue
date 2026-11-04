<script setup lang="ts">
import { useI18n } from "vue-i18n";

import {
  PERSONAL_NAV,
  PersonalNotificationsSection
} from "@/components/settings";

import { DESIGN_PATHS } from "@/router/design";
import SettingsFrame from "./SettingsFrame.vue";

/**
 * 帧 06 — 个人通知设置（§4A Notifications）。
 * 表单体由共享节组件 PersonalNotificationsSection 提供（与应用页共用）。
 */
const { t } = useI18n();

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.personal.landing.title"),
    to: DESIGN_PATHS.settings.personal
  },
  { label: t("settings.nav.notifications") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.notifications')"
    :scope-label="t('settings.personal.landing.title')"
    :entries="PERSONAL_NAV"
  >
    <PersonalNotificationsSection />
  </SettingsFrame>
</template>
