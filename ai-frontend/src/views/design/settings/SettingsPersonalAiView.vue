<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { PERSONAL_NAV, PersonalAiSection } from "@/components/settings";

import { DESIGN_PATHS } from "@/router/design";
import SettingsFrame from "./SettingsFrame.vue";

/**
 * 帧 07 — 个人 AI 偏好（§4A AI Preferences）。
 * 表单体由共享节组件 PersonalAiSection 提供（与应用页共用）。
 */
const { t } = useI18n();

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.personal.landing.title"),
    to: DESIGN_PATHS.settings.personal
  },
  { label: t("settings.nav.ai") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.ai')"
    :scope-label="t('settings.personal.landing.title')"
    :entries="PERSONAL_NAV"
  >
    <PersonalAiSection />
  </SettingsFrame>
</template>
