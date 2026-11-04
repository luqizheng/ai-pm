<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  PERSONAL_NAV,
  SettingsConfirmationDialog,
  SettingsFormRow,
  SettingsFormSection,
  SettingsSaveBar,
  SettingsValidationMessage,
  SESSIONS_FIXTURE,
  type DemoSession
} from "@/components/settings";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Input from "@/components/ui/input/Input.vue";
import Switch from "@/components/ui/switch/Switch.vue";
import Table from "@/components/ui/table/Table.vue";
import TableBody from "@/components/ui/table/TableBody.vue";
import TableCell from "@/components/ui/table/TableCell.vue";
import TableHead from "@/components/ui/table/TableHead.vue";
import TableHeader from "@/components/ui/table/TableHeader.vue";
import TableRow from "@/components/ui/table/TableRow.vue";

import { DESIGN_PATHS } from "@/router/design";
import SettingsFrame from "./SettingsFrame.vue";
import { useDemoForm } from "./useDemoForm";

/**
 * 帧 04 — 个人安全设置（§4A Security）。
 * 密码修改（校验错误演示 §6.5）+ 活跃会话（吊销 = 破坏性确认 §6.7）
 * + 登录安全（2FA/通行密钥为规划中能力，§9 Proposed）。
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
} = useDemoForm(() => ({ current: "", next: "", confirm: "" }));

const validation = computed(() => {
  if (dirty.value) {
    if (!draft.value.current || !draft.value.next) {
      return "settings.personal.security.passwordRequired";
    }
    if (draft.value.next.length < 8) {
      return "settings.personal.security.passwordTooShort";
    }
    if (draft.value.next !== draft.value.confirm) {
      return "settings.personal.security.passwordMismatch";
    }
  }
  return "";
});

/** 会话吊销：候选 + 确认弹窗 + 已吊销结果集 */
const sessions = ref<DemoSession[]>(SESSIONS_FIXTURE);
const revokeTarget = ref<DemoSession | null>(null);
const revokeOpen = computed(() => revokeTarget.value !== null);

function confirmRevoke() {
  if (revokeTarget.value) {
    sessions.value = sessions.value.filter(
      session => session.id !== revokeTarget.value?.id
    );
  }
  revokeTarget.value = null;
}

const twoFactor = ref(false);
const passkeys = ref(false);

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
    <SettingsFormSection
      :title="t('settings.personal.security.passwordTitle')"
      :description="t('settings.personal.security.passwordDesc')"
    >
      <SettingsFormRow
        :label="t('settings.personal.security.currentPassword')"
        html-for="security-current"
        required
      >
        <div class="security-field">
          <Input
            id="security-current"
            v-model="draft.current"
            type="password"
            :invalid="!!validation"
          />
          <SettingsValidationMessage
            v-if="
              validation === 'settings.personal.security.passwordRequired' &&
              !draft.current
            "
            :message="t('settings.personal.security.passwordRequired')"
          />
        </div>
      </SettingsFormRow>
      <SettingsFormRow
        :label="t('settings.personal.security.newPassword')"
        html-for="security-next"
        required
      >
        <div class="security-field">
          <Input
            id="security-next"
            v-model="draft.next"
            type="password"
            :invalid="!!validation"
          />
          <SettingsValidationMessage
            v-if="validation === 'settings.personal.security.passwordTooShort'"
            :message="t('settings.personal.security.passwordTooShort')"
          />
        </div>
      </SettingsFormRow>
      <SettingsFormRow
        :label="t('settings.personal.security.confirmPassword')"
        html-for="security-confirm"
        required
      >
        <div class="security-field">
          <Input
            id="security-confirm"
            v-model="draft.confirm"
            type="password"
            :invalid="!!validation"
          />
          <SettingsValidationMessage
            v-if="validation === 'settings.personal.security.passwordMismatch'"
            :message="t('settings.personal.security.passwordMismatch')"
          />
        </div>
      </SettingsFormRow>
    </SettingsFormSection>

    <SettingsFormSection
      :title="t('settings.personal.security.sessionsTitle')"
      :description="t('settings.personal.security.sessionsDesc')"
    >
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{{ t("settings.personal.security.device") }}</TableHead>
            <TableHead>{{
              t("settings.personal.security.location")
            }}</TableHead>
            <TableHead>{{
              t("settings.personal.security.lastActive")
            }}</TableHead>
            <TableHead>{{ t("settings.personal.security.actions") }}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="session in sessions" :key="session.id">
            <TableCell>
              <span class="security-device">
                {{ session.device }}
                <Badge v-if="session.current" variant="accent" size="sm">
                  {{ t("settings.common.current") }}
                </Badge>
              </span>
            </TableCell>
            <TableCell>{{ session.location }}</TableCell>
            <TableCell>{{ session.lastActive }}</TableCell>
            <TableCell>
              <Button
                v-if="!session.current"
                variant="danger"
                size="sm"
                @click="revokeTarget = session"
              >
                {{ t("settings.personal.security.revoke") }}
              </Button>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </SettingsFormSection>

    <SettingsFormSection :title="t('settings.personal.security.signInTitle')">
      <SettingsFormRow
        :label="t('settings.personal.security.twoFactor')"
        :helper="t('settings.personal.security.twoFactorHelper')"
        :flag="t('settings.common.proposed')"
      >
        <Switch v-model="twoFactor" disabled />
      </SettingsFormRow>
      <SettingsFormRow
        :label="t('settings.personal.security.passkeys')"
        :helper="t('settings.personal.security.passkeysHelper')"
        :flag="t('settings.common.proposed')"
      >
        <Switch v-model="passkeys" disabled />
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

    <SettingsConfirmationDialog
      :open="revokeOpen"
      :title="t('settings.personal.security.revokeTitle')"
      :description="t('settings.personal.security.revokeDesc')"
      :confirm-label="t('settings.personal.security.revoke')"
      @update:open="
        value => {
          if (!value) revokeTarget = null;
        }
      "
      @confirm="confirmRevoke"
    />
  </SettingsFrame>
</template>

<style scoped>
.security-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  max-width: 20rem;
}

.security-device {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-primary);
}
</style>
