<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import {
  TENANT_NAV,
  SettingsFormRow,
  SettingsFormSection,
  SettingsPermissionNotice,
  SettingsReadOnlyField,
  SettingsSaveBar,
  useSettingsDemo
} from "@/components/settings";
import SettingsFrame from "./SettingsFrame.vue";

import Input from "@/components/ui/input/Input.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

import { DESIGN_PATHS } from "@/router/design";
import { useDemoForm } from "./useDemoForm";

/**
 * 帧 09 — 租户基本信息（§4B General）。
 * 管理员可编辑（名称/组织描述），标识只读；
 * 普通成员走 denied 态（§6.9）。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const denied = computed(() => demo.tenantAccess.value === "hidden");

const {
  draft,
  dirty,
  saving,
  saved,
  error,
  save,
  cancel: resetDraft
} = useDemoForm(() => ({
  name: "AI R&D Workspace",
  orgInfo: "Owns engineering and delivery of the AI R&D management platform."
}));

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.tenant.landing.title"),
    to: DESIGN_PATHS.settings.tenant
  },
  { label: t("settings.nav.tenantGeneral") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.tenantGeneral')"
    :scope-label="t('settings.tenant.landing.title')"
    :entries="TENANT_NAV"
    :denied="denied"
    persona="tenant-member"
  >
    <template v-if="!denied">
      <SettingsFormSection :title="t('settings.tenant.general.identityTitle')">
        <SettingsFormRow
          :label="t('settings.tenant.general.tenantName')"
          html-for="tenant-name"
          :helper="t('settings.tenant.general.tenantNameHelper')"
          required
        >
          <Input id="tenant-name" v-model="draft.name" :disabled="denied" />
        </SettingsFormRow>
        <SettingsFormRow
          :label="t('settings.tenant.general.tenantId')"
          :flag="t('settings.common.readOnly')"
          :helper="t('settings.tenant.general.tenantIdHelper')"
        >
          <SettingsReadOnlyField value="tenant_8f3a2c" />
        </SettingsFormRow>
      </SettingsFormSection>

      <SettingsFormSection :title="t('settings.tenant.general.orgTitle')">
        <SettingsFormRow
          :label="t('settings.tenant.general.orgInfo')"
          html-for="tenant-org"
          :helper="t('settings.tenant.general.orgInfoHelper')"
        >
          <Textarea
            id="tenant-org"
            v-model="draft.orgInfo"
            :placeholder="t('settings.tenant.general.orgInfoPlaceholder')"
          />
        </SettingsFormRow>
      </SettingsFormSection>

      <SettingsSaveBar
        :dirty="dirty"
        :saving="saving"
        :saved="saved"
        :error="error ? t('settings.common.saveFailed') : ''"
        @save="save"
        @cancel="resetDraft"
      />
    </template>
  </SettingsFrame>
</template>
