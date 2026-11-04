<script setup lang="ts">
import { useI18n } from "vue-i18n";

import SettingsFormRow from "../SettingsFormRow.vue";
import SettingsFormSection from "../SettingsFormSection.vue";
import SettingsReadOnlyField from "../SettingsReadOnlyField.vue";
import SettingsSaveBar from "../SettingsSaveBar.vue";
import { useDemoForm } from "../useDemoForm";

import Avatar from "@/components/ui/avatar/Avatar.vue";
import Button from "@/components/ui/button/Button.vue";
import Input from "@/components/ui/input/Input.vue";

/**
 * 个人资料节内容（共享）：展示名称（可编辑）+ 头像 +
 * 个人信息（邮箱/租户/加入时间只读）。
 * 设计目录 /design/settings/personal/profile 与应用页
 * /settings/personal/profile 共用同一份表单体（§4A Profile）。
 */
const { t } = useI18n();

const {
  draft,
  dirty,
  saving,
  saved,
  error,
  save,
  cancel: resetDraft
} = useDemoForm(() => ({ displayName: "Han Lin" }));
</script>

<template>
  <SettingsFormSection :title="t('settings.personal.profile.identityTitle')">
    <SettingsFormRow
      :label="t('settings.personal.profile.displayName')"
      html-for="profile-display-name"
      :helper="t('settings.personal.profile.displayNameHelper')"
    >
      <Input
        id="profile-display-name"
        v-model="draft.displayName"
        :placeholder="t('settings.personal.profile.displayNamePlaceholder')"
      />
    </SettingsFormRow>

    <SettingsFormRow
      :label="t('settings.personal.profile.avatar')"
      :helper="t('settings.personal.profile.avatarHelper')"
    >
      <div class="profile-avatar">
        <Avatar name="Han Lin" size="lg" alt="Han Lin" />
        <Button variant="secondary" size="sm">
          {{ t("settings.personal.profile.avatarChange") }}
        </Button>
      </div>
    </SettingsFormRow>
  </SettingsFormSection>

  <SettingsFormSection :title="t('settings.personal.profile.infoTitle')">
    <SettingsFormRow
      :label="t('settings.personal.profile.email')"
      :flag="t('settings.common.readOnly')"
      :helper="t('settings.personal.profile.emailHelper')"
    >
      <SettingsReadOnlyField value="han.lin@example.com" />
    </SettingsFormRow>
    <SettingsFormRow
      :label="t('settings.personal.profile.tenant')"
      :flag="t('settings.common.readOnly')"
      :helper="t('settings.personal.profile.tenantHelper')"
    >
      <SettingsReadOnlyField value="AI R&D Workspace" />
    </SettingsFormRow>
    <SettingsFormRow :label="t('settings.personal.profile.joinedAt')">
      <SettingsReadOnlyField value="2026-08-11" />
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

<style scoped>
.profile-avatar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
</style>
