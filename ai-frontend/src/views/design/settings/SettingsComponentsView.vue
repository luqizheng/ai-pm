<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  SettingsFieldLabel,
  SettingsFormRow,
  SettingsFormSection,
  SettingsHelperText,
  SettingsNavigationItem,
  SettingsPermissionNotice,
  SettingsReadOnlyField,
  SettingsSaveBar,
  SettingsSectionHeader,
  SettingsValidationMessage
} from "@/components/settings";

import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 19 — Settings 组件库（§7 十四组件清单演示）。
 * 弹窗三件（UnsavedChanges / Confirmation）与 SaveBar 已在
 * 帧 18 交互态页演示，此处给出入口链接；其余组件就地呈现。
 */
const { t } = useI18n();

const demoName = ref("Han Lin");

const specimenKeys = [
  "pageLayout",
  "sidebar",
  "navigationItem",
  "sectionHeader",
  "formSection",
  "formRow",
  "fieldLabel",
  "helperText",
  "saveBar",
  "unsavedChangesDialog",
  "permissionNotice",
  "readOnlyField",
  "validationMessage",
  "confirmationDialog"
] as const;
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="scomponents__title">
        {{ t("settings.components.title") }}
      </h1>
      <p class="scomponents__desc">{{ t("settings.components.desc") }}</p>
    </template>

    <p
      v-for="key in specimenKeys"
      :key="`intro-${key}`"
      class="scomponents__index"
    >
      <code>{{ `Settings/${key[0].toUpperCase()}${key.slice(1)}` }}</code>
      {{ t(`settings.components.specimens.${key}`) }}
    </p>

    <div class="scomponents__grid">
      <Specimen
        :label="`Settings/PageLayout — ${t('settings.components.specimens.pageLayout')}`"
        width="100%"
      >
        <RouterLink
          class="scomponents__link"
          :to="DESIGN_PATHS.settings.personal"
        >
          {{ t("settings.personal.landing.title") }} →
        </RouterLink>
      </Specimen>

      <Specimen
        :label="`Settings/Sidebar + NavigationItem — ${t('settings.components.specimens.sidebar')}`"
        width="100%"
      >
        <div class="scomponents__sidebar">
          <SettingsNavigationItem label="Profile" active :frame="'03'" />
          <SettingsNavigationItem label="Notifications" proposed />
          <SettingsNavigationItem label="Integrations" disabled />
        </div>
      </Specimen>

      <Specimen
        :label="`Settings/SectionHeader — ${t('settings.components.specimens.sectionHeader')}`"
        width="100%"
      >
        <SettingsSectionHeader
          :title="t('settings.personal.profile.identityTitle')"
          :description="t('settings.personal.profile.displayNameHelper')"
        />
      </Specimen>

      <Specimen
        :label="`Settings/FormSection + FormRow — ${t('settings.components.specimens.formSection')}`"
        width="100%"
      >
        <SettingsFormSection
          :title="t('settings.personal.profile.identityTitle')"
        >
          <SettingsFormRow
            :label="t('settings.personal.profile.displayName')"
            html-for="kit-display-name"
            required
            :helper="t('settings.personal.profile.displayNameHelper')"
          >
            <input
              id="kit-display-name"
              v-model="demoName"
              class="scomponents__native-input"
              type="text"
            />
          </SettingsFormRow>
        </SettingsFormSection>
      </Specimen>

      <Specimen
        :label="`Settings/FieldLabel — ${t('settings.components.specimens.fieldLabel')}`"
        width="100%"
      >
        <div class="scomponents__labels">
          <SettingsFieldLabel
            :label="t('settings.personal.profile.displayName')"
            required
          />
          <SettingsFieldLabel
            :label="t('settings.tenant.general.tenantId')"
            :flag="t('settings.common.readOnly')"
          />
        </div>
      </Specimen>

      <Specimen
        :label="`Settings/HelperText — ${t('settings.components.specimens.helperText')}`"
        width="100%"
      >
        <SettingsHelperText
          :text="t('settings.personal.profile.displayNameHelper')"
        />
      </Specimen>

      <Specimen
        :label="`Settings/SaveBar — ${t('settings.components.specimens.saveBar')}`"
        width="100%"
      >
        <SettingsSaveBar :dirty="true" @save="() => {}" @cancel="() => {}" />
        <RouterLink
          class="scomponents__link"
          :to="DESIGN_PATHS.settings.states"
        >
          {{ t("settings.states.title") }} →
        </RouterLink>
      </Specimen>

      <Specimen
        :label="`Settings/PermissionNotice — ${t('settings.components.specimens.permissionNotice')}`"
        width="100%"
      >
        <SettingsPermissionNotice
          access="readonly"
          :role="t('settings.demo.persona.projectMember')"
        />
      </Specimen>

      <Specimen
        :label="`Settings/ReadOnlyField — ${t('settings.components.specimens.readOnlyField')}`"
        width="100%"
      >
        <SettingsReadOnlyField value="tenant_8f3a2c" />
      </Specimen>

      <Specimen
        :label="`Settings/ValidationMessage — ${t('settings.components.specimens.validationMessage')}`"
        width="100%"
      >
        <SettingsValidationMessage
          :message="t('settings.personal.security.passwordMismatch')"
        />
      </Specimen>

      <Specimen
        :label="`Settings/UnsavedChangesDialog + ConfirmationDialog — ${t('settings.components.specimens.unsavedChangesDialog')}`"
        width="100%"
      >
        <RouterLink
          class="scomponents__link"
          :to="DESIGN_PATHS.settings.states"
        >
          {{ t("settings.states.unsaved.title") }} /
          {{ t("settings.states.destructive.title") }} →
        </RouterLink>
      </Specimen>
    </div>
  </DesignLayout>
</template>

<style scoped>
.scomponents__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.scomponents__desc {
  margin: var(--space-1) 0 var(--space-4);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.scomponents__index {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0 0 var(--space-1);
  font-size: var(--ws-font-size-micro);
  color: var(--ws-text-secondary);
}

.scomponents__index code {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-accent);
}

.scomponents__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-5);
  margin-top: var(--space-5);
}

.scomponents__sidebar {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.scomponents__labels {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.scomponents__link {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-accent);
  text-decoration: none;
}

.scomponents__link:hover {
  text-decoration: underline;
}

.scomponents__native-input {
  min-height: var(--size-control-md);
  padding: 0 var(--space-3);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-primary);
}

.scomponents__native-input:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}
</style>
