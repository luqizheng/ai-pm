<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { PersonalAiSection } from "@/components/settings";

import { PERSONAL_APP_NAV, PERSONAL_ICONS } from "./nav";
import SettingsAppFrame from "./SettingsAppFrame.vue";

/**
 * 应用设置 — AI 偏好（§4A AI Preferences）。
 * 个人级 AI 行为偏好；租户级 AI 策略不在此处（§9/§10）。
 * 内容为共享节组件。
 */
const { t } = useI18n();

const breadcrumb = [
  {
    label: t("settings.personal.landing.title"),
    to: "/settings/personal"
  },
  { label: t("settings.nav.ai") }
];
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.ai')"
    :scope-label="t('settings.personal.landing.title')"
    :entries="PERSONAL_APP_NAV"
    :icons="PERSONAL_ICONS"
  >
    <PersonalAiSection />
  </SettingsAppFrame>
</template>
