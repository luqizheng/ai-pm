<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  PROJECT_NAV,
  PERSONA_LABEL_KEYS,
  SettingsConfirmationDialog,
  SettingsFormRow,
  SettingsFormSection,
  SettingsPermissionNotice,
  SettingsReadOnlyField,
  SettingsSaveBar,
  useDemoForm,
  useSettingsDemo
} from "@/components/settings";
import SettingsFrame from "./SettingsFrame.vue";

import Button from "@/components/ui/button/Button.vue";
import Input from "@/components/ui/input/Input.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 13 — 项目基本信息（§4C General）。
 * 负责人可编辑 + 危险区（归档 = 破坏性确认）；成员/访问者只读
 * （PermissionNotice + 控件禁用，§6.8/§6.9）。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const editable = computed(
  () => demo.projectAccess.value.general === "editable"
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
  name: "AI R&D Workspace Delivery",
  description:
    "Downstream delivery engineering for the AI R&D management platform."
}));

const archiveOpen = ref(false);

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.project.landing.title"),
    to: DESIGN_PATHS.settings.project
  },
  { label: t("settings.nav.projectGeneral") }
];

const personaLabel = computed(() =>
  t(PERSONA_LABEL_KEYS[demo.state.projectPersona])
);
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.projectGeneral')"
    :scope-label="t('settings.project.landing.title')"
    :entries="PROJECT_NAV"
  >
    <SettingsPermissionNotice
      v-if="!editable"
      access="readonly"
      :role="personaLabel"
    />

    <SettingsFormSection :title="t('settings.project.general.identityTitle')">
      <SettingsFormRow
        :label="t('settings.project.general.projectName')"
        html-for="project-name"
        :helper="t('settings.project.general.projectNameHelper')"
        :required="editable"
      >
        <Input id="project-name" v-model="draft.name" :disabled="!editable" />
      </SettingsFormRow>
      <SettingsFormRow
        :label="t('settings.project.general.projectDesc')"
        html-for="project-desc"
        :helper="t('settings.project.general.projectDescHelper')"
      >
        <Textarea
          id="project-desc"
          v-model="draft.description"
          :disabled="!editable"
          :placeholder="t('settings.project.general.projectDescPlaceholder')"
        />
      </SettingsFormRow>
    </SettingsFormSection>

    <SettingsFormSection :title="t('settings.project.general.metaTitle')">
      <SettingsFormRow
        :label="t('settings.project.general.projectId')"
        :flag="t('settings.common.readOnly')"
        :helper="t('settings.project.general.projectIdHelper')"
      >
        <SettingsReadOnlyField value="proj_5d21e9" />
      </SettingsFormRow>
      <SettingsFormRow :label="t('settings.project.general.createdAt')">
        <SettingsReadOnlyField value="2026-09-01" />
      </SettingsFormRow>
      <SettingsFormRow :label="t('settings.project.general.tenant')">
        <SettingsReadOnlyField value="AI R&D Workspace" />
      </SettingsFormRow>
    </SettingsFormSection>

    <SettingsFormSection
      v-if="editable"
      :title="t('settings.project.general.dangerTitle')"
      :description="t('settings.project.general.dangerDesc')"
    >
      <div class="project-general__danger">
        <Button variant="danger" size="sm" @click="archiveOpen = true">
          {{ t("settings.project.general.archive") }}
        </Button>
      </div>
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

    <SettingsConfirmationDialog
      :open="archiveOpen"
      :title="t('settings.project.general.archiveTitle')"
      :description="t('settings.project.general.archiveDesc')"
      :confirm-label="t('settings.project.general.archive')"
      @update:open="archiveOpen = $event"
      @confirm="archiveOpen = false"
    />
  </SettingsFrame>
</template>

<style scoped>
.project-general__danger {
  padding: var(--space-3);
  border: var(--border-1) solid var(--ws-color-danger-border);
  border-radius: var(--radius-sm);
}
</style>
