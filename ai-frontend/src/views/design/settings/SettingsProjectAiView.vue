<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  PROJECT_NAV,
  PERSONA_LABEL_KEYS,
  SettingsFormRow,
  SettingsFormSection,
  SettingsPermissionNotice,
  SettingsSaveBar,
  useDemoForm,
  useSettingsDemo
} from "@/components/settings";
import SettingsFrame from "./SettingsFrame.vue";

import Alert from "@/components/ui/alert/Alert.vue";
import Radio from "@/components/ui/radio/Radio.vue";
import Switch from "@/components/ui/switch/Switch.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 17 — 项目 AI 配置（§4C AI Configuration）。
 * 执行方式（计划先行/直接执行）+ 通知/上下文开关；
 * 项目专属 AI 策略为规划中；全局 AI 政策在租户治理层（§9/§10 边界）。
 * 人工确认约束高于项目配置 —— 不提供「跳过人工确认」选项。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const editable = computed(() => demo.projectAccess.value.ai === "editable");
const personaLabel = computed(() =>
  t(PERSONA_LABEL_KEYS[demo.state.projectPersona])
);

const {
  draft,
  dirty,
  saving,
  saved,
  error,
  save,
  cancel: resetDraft
} = useDemoForm(() => ({
  execMode: "plan-first" as "plan-first" | "direct",
  notifyComplete: true,
  contextRepo: true
}));

const modeOptions = [
  { value: "plan-first", labelKey: "settings.project.ai.modePlanFirst" },
  { value: "direct", labelKey: "settings.project.ai.modeDirect" }
] as const;

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.project.landing.title"),
    to: DESIGN_PATHS.settings.project
  },
  { label: t("settings.nav.projectAi") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.projectAi')"
    :scope-label="t('settings.project.landing.title')"
    :entries="PROJECT_NAV"
  >
    <SettingsPermissionNotice
      v-if="!editable"
      access="readonly"
      :role="personaLabel"
    />

    <SettingsFormSection
      :title="t('settings.project.ai.executionTitle')"
      :description="t('settings.project.ai.executionDesc')"
    >
      <SettingsFormRow
        :label="t('settings.project.ai.execMode')"
        :helper="t('settings.project.ai.execModeHelper')"
      >
        <div
          class="project-ai__group"
          role="radiogroup"
          :aria-label="t('settings.project.ai.execMode')"
        >
          <Radio
            v-for="option in modeOptions"
            :key="option.value"
            v-model="draft.execMode"
            :value="option.value"
            :label="t(option.labelKey)"
            name="project-ai-mode"
            :disabled="!editable"
          />
        </div>
      </SettingsFormRow>
      <SettingsFormRow :label="t('settings.project.ai.notifyComplete')">
        <Switch
          v-model="draft.notifyComplete"
          :disabled="!editable"
          :aria-label="t('settings.project.ai.notifyComplete')"
        />
      </SettingsFormRow>
      <SettingsFormRow :label="t('settings.project.ai.contextRepo')">
        <Switch
          v-model="draft.contextRepo"
          :disabled="!editable"
          :aria-label="t('settings.project.ai.contextRepo')"
        />
      </SettingsFormRow>
    </SettingsFormSection>

    <SettingsFormSection :title="t('settings.project.ai.policiesTitle')">
      <Alert variant="info" size="sm">
        {{ t("settings.project.ai.policiesBanner") }}
      </Alert>
    </SettingsFormSection>

    <SettingsSaveBar
      v-if="editable"
      :dirty="dirty"
      :saving="saving"
      :saved="saved"
      :error="error ? t('settings.common.saveFailed') : ''"
      @save="save"
      @cancel="resetDraft"
    />
  </SettingsFrame>
</template>

<style scoped>
.project-ai__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
</style>
