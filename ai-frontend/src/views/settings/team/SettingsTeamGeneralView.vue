<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { SettingsFormSection } from "@/components/settings";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  fetchTeamProfile,
  listTeamMembers,
  updateTeamProfile,
  type TeamMember,
  type TeamProfile
} from "@/api/team";
import { useTeamStore } from "@/stores/team";
import SettingsAppFrame from "../SettingsAppFrame.vue";
import { TEAM_APP_NAV, TEAM_ICONS } from "../nav";

/**
 * 团队设置 — 基本（Team Settings & Ownership Transfer，P1）。
 * 档案编辑（Team Owner；slug 冲突回显服务端稳定文案）。P4-ORG：owner-transfer
 * 已取消（多 Owner 任命制，任命/降级在成员管理页）；slug 置空提交 = 清除。
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
const teamId = computed(() => teamStore.context?.team?.id ?? "");

async function reload(): Promise<void> {
  if (!teamId.value) return;
  loading.value = true;
  error.value = null;
  try {
    const [loaded, memberRows] = await Promise.all([
      fetchTeamProfile(teamId.value),
      listTeamMembers(teamId.value)
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
    const saved = await updateTeamProfile(teamId.value, {
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
          label: $t('settings.team.landing.title'),
          to: '/settings/team'
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
            :disabled="!isOwner"
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
            :disabled="!isOwner"
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
            :disabled="!isOwner"
            data-test="team-general-description"
          />
        </FormField>
        <div v-if="isOwner" class="tg-actions">
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

    </SettingsAppFrame>
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

</style>
