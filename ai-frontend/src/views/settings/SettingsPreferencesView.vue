<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { PersonalPreferencesSection } from "@/components/settings";

import { PERSONAL_APP_NAV, PERSONAL_ICONS } from "./nav";
import SettingsAppFrame from "./SettingsAppFrame.vue";

/**
 * 应用设置 — 偏好设置（§4A Preferences）。
 * 入口 = 顶栏账户菜单「个人偏好」；内容为共享节组件。
 * apply-to-app：主题三态真实切换应用主题（html data-theme + aipm.theme 持久化），
 * 设计目录演示页不传该 prop，保持本区块演示语义。
 */
const { t } = useI18n();

const breadcrumb = [
  {
    label: t("settings.personal.landing.title"),
    to: "/settings/personal"
  },
  { label: t("settings.nav.preferences") }
];
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.preferences')"
    :scope-label="t('settings.personal.landing.title')"
    :entries="PERSONAL_APP_NAV"
    :icons="PERSONAL_ICONS"
  >
    <PersonalPreferencesSection apply-to-app />
  </SettingsAppFrame>
</template>
