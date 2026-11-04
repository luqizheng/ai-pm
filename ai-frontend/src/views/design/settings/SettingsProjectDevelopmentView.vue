<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  PROJECT_NAV,
  PERSONA_LABEL_KEYS,
  SettingsFormRow,
  SettingsFormSection,
  SettingsPermissionNotice,
  SettingsReadOnlyField,
  useSettingsDemo
} from "@/components/settings";
import SettingsFrame from "./SettingsFrame.vue";

import Alert from "@/components/ui/alert/Alert.vue";
import Select from "@/components/ui/select/Select.vue";
import Switch from "@/components/ui/switch/Switch.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 16 — 项目开发配置（§4C Development Configuration）。
 * 环境模板（负责人可改）+ 环境变量/部署密钥只读；
 * CI 事件推送为规划中能力（§9 Proposed 横幅 + 禁用开关）。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const editable = computed(
  () => demo.projectAccess.value.development === "editable"
);
const personaLabel = computed(() =>
  t(PERSONA_LABEL_KEYS[demo.state.projectPersona])
);

const template = ref("basic");

const templateOptions = [
  { value: "basic", label: t("settings.project.development.templateBasic") },
  { value: "full", label: t("settings.project.development.templateFull") }
];

const ciWebhook = ref(false);

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.project.landing.title"),
    to: DESIGN_PATHS.settings.project
  },
  { label: t("settings.nav.development") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.development')"
    :scope-label="t('settings.project.landing.title')"
    :entries="PROJECT_NAV"
  >
    <SettingsPermissionNotice
      v-if="!editable"
      access="readonly"
      :role="personaLabel"
    />

    <SettingsFormSection
      :title="t('settings.project.development.title')"
      :description="t('settings.project.development.desc')"
    >
      <SettingsFormRow
        :label="t('settings.project.development.template')"
        :helper="t('settings.project.development.templateHelper')"
      >
        <Select
          v-model="template"
          :options="templateOptions"
          :disabled="!editable"
        />
      </SettingsFormRow>
      <SettingsFormRow
        :label="t('settings.project.development.envVars')"
        :flag="t('settings.common.readOnly')"
        :helper="t('settings.project.development.envVarsHelper')"
      >
        <SettingsReadOnlyField
          value="PM_API_URL=*** · REGISTRY=*** · SSH_KEY=***"
        />
      </SettingsFormRow>
    </SettingsFormSection>

    <SettingsFormSection
      :title="t('settings.project.development.integrationsTitle')"
    >
      <Alert variant="info" size="sm">
        {{ t("settings.project.development.proposedBanner") }}
      </Alert>
      <SettingsFormRow
        :label="t('settings.project.development.ciWebhook')"
        :helper="t('settings.project.development.ciWebhookHelper')"
        :flag="t('settings.common.proposed')"
      >
        <Switch v-model="ciWebhook" disabled />
      </SettingsFormRow>
      <SettingsFormRow
        :label="t('settings.project.development.deployKey')"
        :flag="t('settings.common.readOnly')"
        :helper="t('settings.project.development.deployKeyHelper')"
      >
        <SettingsReadOnlyField value="SHA256:9f4c…E2" />
      </SettingsFormRow>
    </SettingsFormSection>
  </SettingsFrame>
</template>
