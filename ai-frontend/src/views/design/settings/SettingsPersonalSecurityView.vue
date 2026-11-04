<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { PERSONAL_NAV, PersonalSecuritySection } from "@/components/settings";

import { DESIGN_PATHS } from "@/router/design";
import SettingsFrame from "./SettingsFrame.vue";

/**
 * 帧 04 — 个人安全设置（§4A Security）。
 * 表单体由共享节组件 PersonalSecuritySection 提供（与应用页共用）。
 */
const { t } = useI18n();

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.personal.landing.title"),
    to: DESIGN_PATHS.settings.personal
  },
  { label: t("settings.nav.security") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.security')"
    :scope-label="t('settings.personal.landing.title')"
    :entries="PERSONAL_NAV"
  >
    <PersonalSecuritySection />
  </SettingsFrame>
</template>
