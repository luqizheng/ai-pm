<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  PERSONAL_NAV,
  SettingsFormRow,
  SettingsFormSection,
  SettingsPermissionNotice
} from "@/components/settings";

import Select from "@/components/ui/select/Select.vue";
import Switch from "@/components/ui/switch/Switch.vue";

import { DESIGN_PATHS } from "@/router/design";
import SettingsFrame from "./SettingsFrame.vue";

/**
 * 帧 06 — 个人通知设置（§4A Notifications）。
 * 通知类别开关 + 投递频率；邮件通道为规划中能力（§9 Proposed），
 * 以 disabled 开关 + 只读横幅呈现，不冒充已实现功能。
 */
const { t } = useI18n();

const categories = [
  {
    id: "requirement",
    labelKey: "settings.personal.notifications.categoryRequirement",
    helperKey: "settings.personal.notifications.categoryRequirementHelper"
  },
  {
    id: "task",
    labelKey: "settings.personal.notifications.categoryTask",
    helperKey: "settings.personal.notifications.categoryTaskHelper"
  },
  {
    id: "review",
    labelKey: "settings.personal.notifications.categoryReview",
    helperKey: "settings.personal.notifications.categoryReviewHelper"
  },
  {
    id: "ai",
    labelKey: "settings.personal.notifications.categoryAi",
    helperKey: "settings.personal.notifications.categoryAiHelper"
  }
] as const;

const enabled = ref<Record<string, boolean>>({
  requirement: true,
  task: true,
  review: true,
  ai: true
});

const frequency = ref("realtime");
const frequencyOptions = [
  {
    value: "realtime",
    label: t("settings.personal.notifications.frequencyRealtime")
  },
  {
    value: "daily",
    label: t("settings.personal.notifications.frequencyDaily")
  },
  {
    value: "weekly",
    label: t("settings.personal.notifications.frequencyWeekly")
  }
];

const emailMentions = ref(false);
const emailSummary = ref(false);

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
    <SettingsFormSection
      :title="t('settings.personal.notifications.categoriesTitle')"
      :description="t('settings.personal.notifications.categoriesDesc')"
    >
      <SettingsFormRow
        v-for="category in categories"
        :key="category.id"
        :label="t(category.labelKey)"
        :helper="t(category.helperKey)"
      >
        <Switch
          v-model="enabled[category.id]"
          :aria-label="t(category.labelKey)"
        />
      </SettingsFormRow>
    </SettingsFormSection>

    <SettingsFormSection
      :title="t('settings.personal.notifications.deliveryTitle')"
    >
      <SettingsFormRow
        :label="t('settings.personal.notifications.frequency')"
        :helper="t('settings.personal.notifications.frequencyHelper')"
      >
        <Select v-model="frequency" :options="frequencyOptions" />
      </SettingsFormRow>
    </SettingsFormSection>

    <SettingsFormSection
      :title="t('settings.personal.notifications.emailTitle')"
    >
      <SettingsPermissionNotice
        access="readonly"
        :message="t('settings.personal.notifications.emailProposed')"
      />
      <SettingsFormRow
        :label="t('settings.personal.notifications.emailMentions')"
        :flag="t('settings.common.proposed')"
      >
        <Switch v-model="emailMentions" disabled />
      </SettingsFormRow>
      <SettingsFormRow
        :label="t('settings.personal.notifications.emailSummary')"
        :flag="t('settings.common.proposed')"
      >
        <Switch v-model="emailSummary" disabled />
      </SettingsFormRow>
    </SettingsFormSection>
  </SettingsFrame>
</template>
