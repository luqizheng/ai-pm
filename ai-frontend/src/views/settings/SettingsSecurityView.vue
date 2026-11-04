<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { PersonalSecuritySection } from "@/components/settings";

import { PERSONAL_APP_NAV, PERSONAL_ICONS } from "./nav";
import SettingsAppFrame from "./SettingsAppFrame.vue";

/**
 * 应用设置 — 安全（§4A Security）。
 * 入口 = 顶栏账户菜单「安全」；内容为共享节组件。
 */
const { t } = useI18n();

const breadcrumb = [
  {
    label: t("settings.personal.landing.title"),
    to: "/settings/personal"
  },
  { label: t("settings.nav.security") }
];
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.security')"
    :scope-label="t('settings.personal.landing.title')"
    :entries="PERSONAL_APP_NAV"
    :icons="PERSONAL_ICONS"
  >
    <PersonalSecuritySection />
  </SettingsAppFrame>
</template>
