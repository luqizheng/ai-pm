<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { PersonalNotificationsSection } from "@/components/settings";

import { PERSONAL_APP_NAV, PERSONAL_ICONS } from "./nav";
import SettingsAppFrame from "./SettingsAppFrame.vue";

/**
 * 应用设置 — 通知（§4A Notifications）。
 * 与顶栏通知中心（/notifications）分立：此处为通知偏好。
 * 内容为共享节组件。
 */
const { t } = useI18n();

const breadcrumb = [
  {
    label: t("settings.personal.landing.title"),
    to: "/settings/personal"
  },
  { label: t("settings.nav.notifications") }
];
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.notifications')"
    :scope-label="t('settings.personal.landing.title')"
    :entries="PERSONAL_APP_NAV"
    :icons="PERSONAL_ICONS"
  >
    <PersonalNotificationsSection />
  </SettingsAppFrame>
</template>
