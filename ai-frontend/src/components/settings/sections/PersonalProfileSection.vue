<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import SettingsFormRow from "../SettingsFormRow.vue";
import SettingsFormSection from "../SettingsFormSection.vue";
import SettingsReadOnlyField from "../SettingsReadOnlyField.vue";
import SettingsSaveBar from "../SettingsSaveBar.vue";

import {
  getMe,
  resolveAvatarSrc,
  updateMe,
  uploadAvatar,
  type SelfProfile
} from "@/api/base/self-service";
import Avatar from "@/components/ui/avatar/Avatar.vue";
import Button from "@/components/ui/button/Button.vue";
import Input from "@/components/ui/input/Input.vue";
import { useAuthStore } from "@/stores/auth";

/**
 * 个人资料节内容（共享）：展示名称可编辑、头像可上传；
 * 邮箱/租户/加入时间只读（邮箱变更需要验证流程，本轮不做）。
 * 设计目录 /design/settings/personal/profile 与应用页
 * /settings/personal/profile 共用同一份表单体（§4A Profile）。
 * 保存成功后把新资料同步进 auth store，顶栏账户菜单随即更新。
 */
const { t } = useI18n();
const auth = useAuthStore();

const profile = ref<SelfProfile | null>(null);
const displayName = ref("");
const avatarSrc = ref<string | undefined>(undefined);
const loading = ref(true);
const saving = ref(false);
const saved = ref(false);
const error = ref(false);
const avatarInput = ref<HTMLInputElement | null>(null);

const dirty = computed(
  () => profile.value !== null && displayName.value !== profile.value.nickname
);

const email = computed(() => profile.value?.email ?? "");
const joinedAt = computed(() => profile.value?.created_at?.slice(0, 10) ?? "");

async function load() {
  loading.value = true;
  error.value = false;
  try {
    const me = await getMe();
    profile.value = me;
    displayName.value = me.nickname;
    avatarSrc.value = await resolveAvatarSrc(me.avatar);
  } catch {
    error.value = true;
  } finally {
    loading.value = false;
  }
}

function cancel() {
  if (profile.value) {
    displayName.value = profile.value.nickname;
  }
}

async function save() {
  saving.value = true;
  error.value = false;
  try {
    const me = await updateMe({ nickname: displayName.value.trim() });
    profile.value = me;
    displayName.value = me.nickname;
    auth.applyProfile(me);
    saved.value = true;
    window.setTimeout(() => {
      saved.value = false;
    }, 2000);
  } catch {
    error.value = true;
  } finally {
    saving.value = false;
  }
}

function pickAvatar() {
  avatarInput.value?.click();
}

async function onAvatarChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const picked = input.files?.[0];
  input.value = "";
  if (!picked) return;

  saving.value = true;
  error.value = false;
  try {
    const url = await uploadAvatar(picked);
    const me = await updateMe({ avatar: url });
    profile.value = me;
    avatarSrc.value = await resolveAvatarSrc(me.avatar);
    auth.applyProfile(me);
    saved.value = true;
    window.setTimeout(() => {
      saved.value = false;
    }, 2000);
  } catch {
    error.value = true;
  } finally {
    saving.value = false;
  }
}

onMounted(load);
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
        v-model="displayName"
        :placeholder="t('settings.personal.profile.displayNamePlaceholder')"
        :disabled="loading"
      />
    </SettingsFormRow>

    <SettingsFormRow
      :label="t('settings.personal.profile.avatar')"
      :helper="t('settings.personal.profile.avatarHelper')"
    >
      <div class="profile-avatar">
        <Avatar
          :src="avatarSrc"
          :name="displayName || profile?.username || ''"
          size="lg"
        />
        <Button
          variant="secondary"
          size="sm"
          :loading="saving"
          @click="pickAvatar"
        >
          {{ t("settings.personal.profile.avatarChange") }}
        </Button>
        <input
          ref="avatarInput"
          type="file"
          accept="image/*"
          class="profile-avatar__input"
          @change="onAvatarChange"
        />
      </div>
    </SettingsFormRow>
  </SettingsFormSection>

  <SettingsFormSection :title="t('settings.personal.profile.infoTitle')">
    <SettingsFormRow
      :label="t('settings.personal.profile.email')"
      :flag="t('settings.common.readOnly')"
      :helper="t('settings.personal.profile.emailHelper')"
    >
      <SettingsReadOnlyField :value="email" />
    </SettingsFormRow>
    <SettingsFormRow
      :label="t('settings.personal.profile.tenant')"
      :flag="t('settings.common.readOnly')"
      :helper="t('settings.personal.profile.tenantHelper')"
    >
      <SettingsReadOnlyField value="AI R&D Workspace" />
    </SettingsFormRow>
    <SettingsFormRow :label="t('settings.personal.profile.joinedAt')">
      <SettingsReadOnlyField :value="joinedAt" />
    </SettingsFormRow>
  </SettingsFormSection>

  <SettingsSaveBar
    :dirty="dirty"
    :saving="saving"
    :saved="saved"
    :error="error ? t('settings.common.saveFailed') : ''"
    @save="save"
    @cancel="cancel"
  />
</template>

<style scoped>
.profile-avatar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.profile-avatar__input {
  display: none;
}
</style>
