<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import SettingsFormRow from "../SettingsFormRow.vue";
import SettingsFormSection from "../SettingsFormSection.vue";

import Alert from "@/components/ui/alert/Alert.vue";
import Radio from "@/components/ui/radio/Radio.vue";
import Switch from "@/components/ui/switch/Switch.vue";

/**
 * 个人 AI 偏好节内容（共享）：交互方式（人工确认不可跳过，§6 铁律）
 * + 显示偏好；边界说明：租户级 AI 策略不在此处（§9/§10）。
 * 设计目录 /design/settings/personal/ai 与应用页
 * /settings/personal/ai 共用同一份表单体（§4A AI Preferences）。
 */
const { t } = useI18n();

const interactionStyle = ref<"suggest" | "confirm" | "auto">("confirm");

const styleOptions = [
  { value: "suggest", labelKey: "settings.personal.ai.styleSuggest" },
  { value: "confirm", labelKey: "settings.personal.ai.styleConfirm" },
  { value: "auto", labelKey: "settings.personal.ai.styleAuto" }
] as const;

const showReasoning = ref(true);
const inlineSuggestions = ref(true);
const proactiveTips = ref(false);
</script>

<template>
  <SettingsFormSection
    :title="t('settings.personal.ai.behaviorTitle')"
    :description="t('settings.personal.ai.behaviorDesc')"
  >
    <SettingsFormRow
      :label="t('settings.personal.ai.interactionStyle')"
      :helper="t('settings.personal.ai.interactionStyleHelper')"
    >
      <div
        class="personal-ai__group"
        role="radiogroup"
        :aria-label="t('settings.personal.ai.interactionStyle')"
      >
        <Radio
          v-for="option in styleOptions"
          :key="option.value"
          v-model="interactionStyle"
          :value="option.value"
          :label="t(option.labelKey)"
          name="personal-ai-style"
        />
      </div>
    </SettingsFormRow>
  </SettingsFormSection>

  <SettingsFormSection :title="t('settings.personal.ai.displayTitle')">
    <SettingsFormRow :label="t('settings.personal.ai.showReasoning')">
      <Switch v-model="showReasoning" />
    </SettingsFormRow>
    <SettingsFormRow :label="t('settings.personal.ai.inlineSuggestions')">
      <Switch v-model="inlineSuggestions" />
    </SettingsFormRow>
    <SettingsFormRow :label="t('settings.personal.ai.proactiveTips')">
      <Switch v-model="proactiveTips" />
    </SettingsFormRow>
  </SettingsFormSection>

  <Alert variant="info" size="sm">
    {{ t("settings.personal.ai.boundary") }}
  </Alert>
</template>

<style scoped>
.personal-ai__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
</style>
