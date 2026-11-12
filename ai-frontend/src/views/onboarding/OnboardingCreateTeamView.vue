<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Info } from "lucide-vue-next";

import OnboardingShell from "./OnboardingShell.vue";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { createTeam, type TeamProfile } from "@/api/team";
import { translateError } from "@/api/base";
import { useTeamStore } from "@/stores/team";

/**
 * Screen 02 — Create Team（设计稿 §4）：团队名必填 / slug 自动建议（可改）/
 * 描述可选；创建者自动成为 Owner（Q2）。校验五态：必填/slug 重复(slug_already_taken)/
 * 非法字符/网络错误/创建失败（translateError 兜底）。
 */
const { t } = useI18n();
const router = useRouter();
const teamStore = useTeamStore();

const name = ref("");
const slug = ref("");
const description = ref("");
const slugEdited = ref(false);
const submitting = ref(false);
const submitError = ref<string | null>(null);

/** slug 自动建议：小写化 + 空白/非法字符折叠为连字符 */
const suggestedSlug = computed(() => {
  const base = name.value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 100);
  return base;
});

function onNameInput(): void {
  if (!slugEdited.value) slug.value = suggestedSlug.value;
}

const nameError = computed(() =>
  name.value.trim() ? null : t("team.onboarding.create.nameRequired")
);
const slugError = computed(() => {
  if (!slug.value) return null; // 可选
  if (!/^[a-z0-9][a-z0-9-]{0,99}$/.test(slug.value))
    return t("team.onboarding.create.slugInvalid");
  return null;
});

const canSubmit = computed(
  () => name.value.trim().length > 0 && !slugError.value && !submitting.value
);

function slugToError(message: string): string {
  return message === "slug_already_taken"
    ? t("team.onboarding.create.slugTaken")
    : message;
}

async function submit(): Promise<void> {
  if (!canSubmit.value) return;
  submitting.value = true;
  submitError.value = null;
  try {
    const team: TeamProfile = await createTeam({
      name: name.value.trim(),
      slug: slug.value.trim() || undefined,
      description: description.value.trim() || undefined
    });
    await teamStore.refresh();
    void router.push({
      path: "/onboarding/invite",
      query: { created: team.id }
    });
  } catch (err) {
    submitError.value = slugToError(translateError(err));
  } finally {
    submitting.value = false;
  }
}

function back(): void {
  void router.push("/onboarding");
}
</script>

<template>
  <OnboardingShell>
    <div class="create">
      <h1 class="create__title">{{ t("team.onboarding.create.title") }}</h1>
      <p class="create__desc">{{ t("team.onboarding.create.desc") }}</p>

      <form class="create__form" @submit.prevent="submit">
        <FormField
          :label="t('team.onboarding.create.nameLabel')"
          required
          :error="nameError ?? undefined"
          html-for="team-name"
        >
          <Input
            id="team-name"
            v-model="name"
            data-testid="onboarding-team-name"
            :placeholder="t('team.onboarding.create.namePlaceholder')"
            :maxlength="120"
            @input="onNameInput"
          />
        </FormField>

        <FormField
          :label="t('team.onboarding.create.slugLabel')"
          :description="t('team.onboarding.create.slugHint')"
          :error="slugError ?? undefined"
          html-for="team-slug"
        >
          <Input
            id="team-slug"
            v-model="slug"
            data-testid="onboarding-team-slug"
            :placeholder="t('team.onboarding.create.slugPlaceholder')"
            :maxlength="100"
            @input="slugEdited = true"
          />
        </FormField>

        <FormField
          :label="t('team.onboarding.create.descLabel')"
          html-for="team-description"
        >
          <Textarea
            id="team-description"
            v-model="description"
            data-testid="onboarding-team-description"
            :placeholder="t('team.onboarding.create.descPlaceholder')"
            :rows="3"
          />
        </FormField>

        <p class="create__notice">
          <Info :size="14" aria-hidden="true" />
          {{ t("team.onboarding.create.ownerNotice") }}
        </p>

        <p
          v-if="submitError"
          class="create__error"
          data-testid="onboarding-create-error"
        >
          {{ submitError }}
        </p>

        <div class="create__actions">
          <Button
            variant="secondary"
            type="button"
            data-testid="onboarding-create-back"
            @click="back"
          >
            {{ t("team.onboarding.action.back") }}
          </Button>
          <Button
            variant="primary"
            type="submit"
            :loading="submitting"
            :disabled="!canSubmit"
            data-testid="onboarding-create-submit"
          >
            {{ t("team.onboarding.create.submit") }}
          </Button>
        </div>
      </form>
    </div>
  </OnboardingShell>
</template>

<style scoped>
.create {
  width: 100%;
  max-width: 520px;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding-top: var(--space-6);
}

.create__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h3);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.create__desc {
  margin: 0 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
}

.create__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.create__notice {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.create__notice svg {
  color: var(--text-secondary);
  flex-shrink: 0;
}

.create__error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.create__actions {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
}
</style>
