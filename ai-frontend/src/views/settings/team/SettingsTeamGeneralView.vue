<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { SettingsFormSection } from "@/components/settings";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  fetchTeamProfile,
  listTeamMembers,
  transferTeamOwnership,
  updateTeamProfile,
  type TeamMember,
  type TeamProfile
} from "@/api/team";
import { useTeamStore } from "@/stores/team";
import SettingsAppFrame from "../SettingsAppFrame.vue";
import { TEAM_APP_NAV, TEAM_ICONS } from "../nav";

/**
 * 团队设置 — 基本（Team Settings & Ownership Transfer，P1）。
 * 档案编辑（owner/admin；slug 冲突回显服务端稳定文案）；Owner 转移卡
 * （仅 owner 可见；目标必须已是 ACTIVE 成员；确认对话框明示原 Owner 降 ADMIN）。
 * slug 置空提交 = 清除。
 */
const { t } = useI18n();
const toast = useToast();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const saving = ref(false);
const profile = ref<TeamProfile | null>(null);
const members = ref<TeamMember[]>([]);

const form = reactive({ name: "", slug: "", description: "" });

const isOwner = computed(() => teamStore.teamRole === "owner");
const isAdmin = computed(() => teamStore.isTeamAdmin);
const tenantId = computed(() => teamStore.context?.tenant_id ?? "");
const selfId = computed(() => teamStore.context?.principal_id ?? "");

/** 可接任目标：ACTIVE 成员、非 owner、非本人 */
const transferTargets = computed(() =>
  members.value.filter(
    member =>
      member.role !== "owner" &&
      member.is_active &&
      member.principal_id !== selfId.value
  )
);

const transferTargetId = ref<string>("");
const transferOpen = ref(false);
const transferring = ref(false);
const transferTargetName = computed(
  () =>
    transferTargets.value.find(
      member => member.principal_id === transferTargetId.value
    )?.display_name ?? ""
);

async function reload(): Promise<void> {
  if (!tenantId.value) return;
  loading.value = true;
  error.value = null;
  try {
    const [loaded, memberRows] = await Promise.all([
      fetchTeamProfile(tenantId.value),
      listTeamMembers(tenantId.value)
    ]);
    profile.value = loaded;
    members.value = memberRows;
    form.name = loaded.name;
    form.slug = loaded.slug ?? "";
    form.description = loaded.description ?? "";
  } catch (err) {
    error.value = translateError(err);
  } finally {
    loading.value = false;
  }
}

async function save(): Promise<void> {
  if (!form.name.trim() || saving.value) return;
  saving.value = true;
  error.value = null;
  try {
    const saved = await updateTeamProfile(tenantId.value, {
      name: form.name.trim(),
      slug: form.slug.trim() || null,
      description: form.description.trim() || null
    });
    profile.value = saved;
    form.name = saved.name;
    form.slug = saved.slug ?? "";
    form.description = saved.description ?? "";
    toast.toast({ title: t("team.settings.savedToast") });
    await teamStore.refresh();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    saving.value = false;
  }
}

async function confirmTransfer(): Promise<void> {
  if (!transferTargetId.value || transferring.value) return;
  transferring.value = true;
  try {
    await transferTeamOwnership(tenantId.value, transferTargetId.value);
    toast.toast({
      title: t("team.settings.transferredToast", {
        name: transferTargetName.value
      })
    });
    transferOpen.value = false;
    await Promise.all([reload(), teamStore.refresh()]);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    transferring.value = false;
  }
}

onMounted(async () => {
  // 团队上下文可能未加载（深链/测试）：先拉 /v1/pm/me 再取本域数据。
  await teamStore.load();
  await reload();
});
</script>

<template>
  <div class="settings-shell-pad">
    <SettingsAppFrame
      :breadcrumb="[
        {
          label: $t('settings.personal.landing.title'),
          to: '/settings/personal'
        },
        { label: $t('team.settings.title') }
      ]"
      :title="$t('team.settings.title')"
      :description="$t('team.settings.description')"
      :scope-label="$t('settings.team.landing.title')"
      :entries="TEAM_APP_NAV"
      :icons="TEAM_ICONS"
    >
      <p v-if="error" class="tg-error">{{ error }}</p>

      <SettingsFormSection
        :title="$t('team.settings.profileTitle')"
        :description="$t('team.settings.profileDesc')"
      >
        <FormField
          :label="$t('team.onboarding.create.nameLabel')"
          required
          html-for="team-name"
        >
          <Input
            id="team-name"
            v-model="form.name"
            :maxlength="120"
            :disabled="!isAdmin"
            data-test="team-general-name"
          />
        </FormField>
        <FormField
          :label="$t('team.onboarding.create.slugLabel')"
          :description="$t('team.onboarding.create.slugHint')"
          html-for="team-slug"
        >
          <Input
            id="team-slug"
            v-model="form.slug"
            :maxlength="100"
            :disabled="!isAdmin"
            data-test="team-general-slug"
          />
        </FormField>
        <FormField
          :label="$t('team.onboarding.create.descLabel')"
          html-for="team-description"
        >
          <Textarea
            id="team-description"
            v-model="form.description"
            :rows="3"
            :disabled="!isAdmin"
            data-test="team-general-description"
          />
        </FormField>
        <div v-if="isAdmin" class="tg-actions">
          <Button
            variant="primary"
            size="sm"
            :loading="saving"
            :disabled="!form.name.trim()"
            data-test="team-general-save"
            @click="save"
          >
            {{ $t("team.settings.save") }}
          </Button>
        </div>
        <p v-else class="tg-note">{{ $t("team.members.readonlyNote") }}</p>
      </SettingsFormSection>

      <!-- Owner 转移（仅 owner 可见；无权限时不渲染入口 = 细则 #3） -->
      <SettingsFormSection
        v-if="isOwner"
        :title="$t('team.settings.transferTitle')"
        :description="$t('team.settings.transferDesc')"
      >
        <div class="tg-transfer">
          <select
            v-model="transferTargetId"
            class="tg-transfer__select"
            :aria-label="$t('team.settings.transferTarget')"
            data-test="team-transfer-target"
          >
            <option value="" disabled>
              {{ $t("team.settings.transferTarget") }}
            </option>
            <option
              v-for="member in transferTargets"
              :key="member.principal_id"
              :value="member.principal_id"
            >
              {{ member.display_name }} · {{ member.role.toUpperCase() }}
            </option>
          </select>
          <Badge v-if="profile" variant="accent">{{
            $t("team.settings.currentOwner")
          }}</Badge>
          <Button
            variant="primary"
            size="sm"
            :disabled="!transferTargetId"
            data-test="team-transfer-open"
            @click="transferOpen = true"
          >
            {{ $t("team.settings.transferAction") }}
          </Button>
        </div>
      </SettingsFormSection>
    </SettingsAppFrame>

    <Dialog
      :open="transferOpen"
      :title="
        $t('team.settings.transferConfirmTitle', { name: transferTargetName })
      "
      :description="$t('team.settings.transferConfirmDesc')"
      size="sm"
      @update:open="
        value => {
          transferOpen = value;
        }
      "
    >
      <template #footer>
        <Button variant="secondary" size="sm" @click="transferOpen = false">
          {{ $t("team.members.cancel") }}
        </Button>
        <Button
          variant="primary"
          size="sm"
          :loading="transferring"
          data-test="team-transfer-confirm"
          @click="confirmTransfer"
        >
          {{ $t("team.settings.transferAction") }}
        </Button>
      </template>
    </Dialog>
  </div>
</template>

<style scoped>
.tg-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.tg-actions {
  display: flex;
  justify-content: flex-end;
}

.tg-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.tg-transfer {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.tg-transfer__select {
  min-width: 220px;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
}
</style>
