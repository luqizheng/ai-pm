<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import {
  listTeamMembers,
  setTeamMemberRole,
  type TeamMember
} from "@/api/team";
import { useTeamStore } from "@/stores/team";
import TeamDomainTabs from "@/views/team/TeamDomainTabs.vue";

/**
 * Team Owner 继任（P4-ORG UX 修订 V1.0 / Figma T3「Owner succession」）。
 *
 * 三步流程：选定继任者 → 复核影响 → 应用变更。成功语义（T5）：继任者成为
 * / 保持 Team Owner，当前 Owner（调用主体）降为 Team Member。落库顺序 =
 * 先任命后降级——任一步失败都不会出现零 Owner（末位 Owner 保护由服务端
 * 强制，此处以顺序冗余兜底）。仅 Team Owner 可进入；非 Owner 回成员页。
 */
const { t } = useI18n();
const router = useRouter();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const members = ref<TeamMember[]>([]);
const successorId = ref("");
const submitting = ref(false);

const isOwner = computed(() => teamStore.teamRole === "owner");
const teamId = computed(() => teamStore.context?.team?.id ?? "");
const selfId = computed(() => teamStore.context?.principal_id ?? "");
const selfName = computed(() => teamStore.context?.display_name ?? "");

/** 合格继任者 = 自己之外的已激活成员（现任 Owner 也可被选，见设计稿） */
const eligibleSuccessors = computed(() =>
  members.value.filter(
    member => member.is_active && member.principal_id !== selfId.value
  )
);

const successorOptions = computed(() =>
  eligibleSuccessors.value.map(member => ({
    value: member.principal_id,
    label:
      member.role === "owner"
        ? `${member.display_name} · ${t("team.succession.successorOwnerSuffix")}`
        : member.display_name
  }))
);

const successor = computed(() =>
  eligibleSuccessors.value.find(
    member => member.principal_id === successorId.value
  )
);

/** 变更后团队保留的 Owner 数：现任 Owner 中移除自己，继任者若原是成员则补入 */
const ownersAfterChange = computed(() => {
  const owners = members.value.filter(
    member => member.role === "owner" && member.is_active
  ).length;
  const successorWasOwner = successor.value?.role === "owner";
  return Math.max(0, owners - 1 + (successorWasOwner ? 0 : 1));
});

const steps = computed(() => [
  {
    key: "select",
    label: t("team.succession.stepSelect"),
    done: successorId.value !== ""
  },
  { key: "review", label: t("team.succession.stepReview"), done: false },
  { key: "apply", label: t("team.succession.stepApply"), done: false }
]);

async function reload(): Promise<void> {
  if (!teamId.value) return;
  loading.value = true;
  error.value = null;
  try {
    members.value = await listTeamMembers(teamId.value);
  } catch (err) {
    error.value =
      err instanceof Error ? err.message : t("team.succession.loadFailed");
  } finally {
    loading.value = false;
  }
}

async function confirmSuccession(): Promise<void> {
  if (!successor.value || submitting.value) return;
  submitting.value = true;
  const target = successor.value;
  try {
    // 先任命后降级：中途失败时团队仍保有 Owner（零 Owner 态不可能出现）。
    await setTeamMemberRole(teamId.value, target.principal_id, "owner");
    try {
      await setTeamMemberRole(teamId.value, selfId.value, "member");
    } catch {
      // 降级失败：继任者已是 Owner，团队处于多 Owner 合法态；如实回成员页。
    }
    void router.push({
      path: "/settings/team/members",
      query: {
        owner_changed: "1",
        owner_changed_from: selfName.value,
        owner_changed_to: target.display_name
      }
    });
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    submitting.value = false;
  }
}

function cancel(): void {
  void router.push("/settings/team/members");
}

onMounted(async () => {
  await teamStore.load();
  // 角色即权限：非 Team Owner 直接回成员页（服务端同矩阵强制）。
  if (!isOwner.value) {
    void router.replace("/settings/team/members");
    return;
  }
  await reload();
});
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: $t('team.tabs.domain') },
      { label: teamStore.context?.team?.name ?? '', to: '/dashboard' },
      { label: $t('team.succession.title') }
    ]"
    :title="$t('team.succession.title')"
  >
    <TeamDomainTabs />

    <!-- 三步流程指示（T3：1 Select successor → 2 Review → 3 Apply change） -->
    <ol class="ts-steps" data-testid="succession-steps">
      <template v-for="(step, index) in steps" :key="step.key">
        <li
          class="ts-steps__item"
          :class="{ 'is-done': step.done }"
          :aria-current="step.done ? 'step' : undefined"
        >
          <span class="ts-steps__num">{{ index + 1 }}</span>
          {{ step.label }}
        </li>
        <li
          v-if="index < steps.length - 1"
          class="ts-steps__arrow"
          aria-hidden="true"
        >
          →
        </li>
      </template>
    </ol>

    <section class="ts-card">
      <h2 class="ts-card__title">{{ $t("team.succession.cardTitle") }}</h2>
      <p class="ts-card__heading" data-testid="succession-heading">
        {{ selfName }} →
        {{ successor ? successor.display_name : "—" }}
      </p>

      <div class="ts-field">
        <Select
          v-model="successorId"
          :options="successorOptions"
          :placeholder="$t('team.succession.successorPlaceholder')"
          :aria-label="$t('team.succession.successorLabel')"
          :data-test="'succession-successor-select'"
        />
        <p class="ts-field__help">{{ $t("team.succession.successorHelp") }}</p>
      </div>

      <h3 class="ts-card__section">
        {{ $t("team.succession.afterThisChange") }}
      </h3>
      <div class="ts-impact">
        <p v-if="successor" data-testid="succession-impact-successor">
          {{
            successor.role === "owner"
              ? $t("team.succession.impactSuccessorRemains", {
                  name: successor.display_name
                })
              : $t("team.succession.impactSuccessorBecomes", {
                  name: successor.display_name
                })
          }}
          {{
            $t("team.succession.impactSelfBecomesMember", { name: selfName })
          }}
        </p>
        <p data-testid="succession-impact-retain">
          {{
            $t("team.succession.impactRetain", {
              count: ownersAfterChange
            })
          }}
          {{ $t("team.succession.impactProducts") }}
        </p>
      </div>

      <div class="ts-actions">
        <Button
          variant="primary"
          size="sm"
          :disabled="!successor"
          :loading="submitting"
          data-test="succession-confirm"
          @click="confirmSuccession"
        >
          {{ $t("team.succession.confirm") }}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          :disabled="submitting"
          data-test="succession-cancel"
          @click="cancel"
        >
          {{ $t("team.succession.cancel") }}
        </Button>
      </div>
    </section>

    <p v-if="error" class="ts-error">{{ error }}</p>

    <Alert variant="warning" :title="$t('team.succession.lastOwnerTitle')">
      {{ $t("team.succession.lastOwnerDetail") }}
    </Alert>
  </PageFrame>
</template>

<style scoped>
.ts-steps {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.ts-steps__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.ts-steps__item.is-done {
  color: var(--text-primary);
}

.ts-steps__num {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.25rem;
  height: 1.25rem;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-full);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.ts-steps__item.is-done .ts-steps__num {
  border-color: var(--ws-accent);
  color: var(--ws-accent);
}

.ts-steps__arrow {
  color: var(--text-tertiary);
}

.ts-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.ts-card__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ts-card__heading {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ts-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  max-width: 36rem;
}

.ts-field__help {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.ts-card__section {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ts-impact {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ts-impact p {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.ts-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.ts-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}
</style>
