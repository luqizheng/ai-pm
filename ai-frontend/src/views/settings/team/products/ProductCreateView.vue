<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { createProduct, type ProductProfile } from "@/api/products";
import { listTeamMembers, type TeamMember } from "@/api/team";
import { useAuthStore } from "@/stores/auth";
import { useTeamStore } from "@/stores/team";
import { deriveProductKeyPreview } from "./product-display";

/**
 * 创建 Product 流程页（P4-ORG UX 修订 / Figma C1「Create Product」→
 * C2「Select first Owner」→ C3「Review Product」）。
 *
 * 三步同路由向导（草稿存活于组件态）：1 Basic information → 2 First Owner
 * → 3 Confirm。与 T 系列同口径的架构适配：
 *   - Product key 为**只读派生预览**（PA-07 §4.8：服务端自动生成、创建后
 *     不可变、全局唯一）——设计稿的输入框按架构决策改为不可编辑；
 *   - Owning Team 固定为当前团队（disabled 字段）；
 *   - First Owner 必选（C0 校验错态 = 未选时 Review 阻断，与设计稿一致）。
 * 仅 Team Owner 可进入（角色即权限，服务端同矩阵强制）。
 */
const { t } = useI18n();
const router = useRouter();
const auth = useAuthStore();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const members = ref<TeamMember[]>([]);
const submitting = ref(false);

/** 1 = Basic information / 2 = First Owner / 3 = Confirm */
const step = ref<1 | 2 | 3>(1);
const draftName = ref("");
const draftOwnerId = ref("");

const isOwner = computed(() => teamStore.teamRole === "owner");
const teamId = computed(() => teamStore.context?.team?.id ?? "");
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const organization = computed(() => auth.organization ?? "");
const selfName = computed(() => teamStore.context?.display_name ?? "");

/** key 派生预览（权威值由服务端落库时生成，含冲突后缀） */
const keyPreview = computed(() => deriveProductKeyPreview(draftName.value));

const stepLabels = computed(() => [
  t("team.products.stepBasics"),
  t("team.products.stepOwner"),
  t("team.products.stepConfirm")
]);

const eligibleOwners = computed(() =>
  members.value.filter(member => member.is_active)
);

const ownerOptions = computed(() =>
  eligibleOwners.value.map(member => ({
    value: member.principal_id,
    label: `${member.display_name} · ${teamName.value}`
  }))
);

const selectedOwner = computed(() =>
  eligibleOwners.value.find(
    member => member.principal_id === draftOwnerId.value
  )
);

const stepTitleKeys: Record<number, string> = {
  1: "team.products.stepBasicsTitle",
  2: "team.products.stepOwnerTitle",
  3: "team.products.stepReviewTitle"
};

async function reload(): Promise<void> {
  if (!teamId.value) return;
  loading.value = true;
  error.value = null;
  try {
    members.value = await listTeamMembers(teamId.value);
  } catch (err) {
    error.value =
      err instanceof Error ? err.message : t("team.products.loadFailed");
  } finally {
    loading.value = false;
  }
}

function goToList(): void {
  void router.push("/settings/team/products");
}

async function submitCreate(): Promise<void> {
  if (!draftName.value.trim() || !selectedOwner.value || submitting.value)
    return;
  submitting.value = true;
  error.value = null;
  try {
    const created: ProductProfile = await createProduct(teamId.value, {
      name: draftName.value.trim(),
      first_owner_principal_id: draftOwnerId.value
    });
    void router.push(`/settings/team/products/${created.id}`);
  } catch (err) {
    error.value =
      err instanceof Error ? err.message : t("team.products.createFailed");
  } finally {
    submitting.value = false;
  }
}

onMounted(async () => {
  await teamStore.load();
  // 角色即权限：非 Team Owner 直接回列表（服务端同矩阵强制）。
  if (!isOwner.value) {
    void router.replace("/settings/team/products");
    return;
  }
  await reload();
});
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: $t('team.tabs.domain') },
      { label: teamName, to: '/dashboard' },
      { label: $t('team.products.title'), to: '/settings/team/products' },
      { label: $t(stepTitleKeys[step]) }
    ]"
    :title="$t(stepTitleKeys[step])"
    :description="
      organization
        ? $t('team.products.listSubtitle', {
            team: teamName,
            org: organization
          })
        : undefined
    "
  >
    <!-- 三步流程指示（C1/C2/C3 共用） -->
    <ol class="pc-steps" data-testid="create-steps">
      <template v-for="(label, index) in stepLabels" :key="label">
        <li
          class="pc-steps__item"
          :class="{ 'is-current': step === index + 1 }"
          :aria-current="step === index + 1 ? 'step' : undefined"
        >
          <span class="pc-steps__num">{{ index + 1 }}</span>
          {{ label }}
        </li>
        <li
          v-if="index < stepLabels.length - 1"
          class="pc-steps__arrow"
          aria-hidden="true"
        >
          →
        </li>
      </template>
    </ol>

    <p v-if="error" class="pc-error">{{ error }}</p>

    <!-- 第 1 步：Basic information（C1） -->
    <section v-if="step === 1" class="pc-card">
      <h2 class="pc-card__title">{{ $t("team.products.basicsTitle") }}</h2>
      <div class="pc-field">
        <label class="pc-field__label" for="product-create-name">
          {{ $t("team.products.fieldName") }}
        </label>
        <Input
          id="product-create-name"
          v-model="draftName"
          :maxlength="120"
          :placeholder="$t('team.products.namePlaceholder')"
          data-test="create-name"
        />
        <p class="pc-field__help">{{ $t("team.products.fieldNameHelp") }}</p>
      </div>
      <div class="pc-field">
        <label class="pc-field__label" for="product-create-key">
          {{ $t("team.products.fieldKey") }}
        </label>
        <Input
          id="product-create-key"
          :model-value="keyPreview"
          disabled
          :placeholder="$t('team.products.keyPreviewPlaceholder')"
          data-test="create-key-preview"
        />
        <p class="pc-field__help">{{ $t("team.products.fieldKeyHelp") }}</p>
      </div>
      <div class="pc-field">
        <span class="pc-field__label">{{ $t("team.products.fieldTeam") }}</span>
        <div class="pc-field__static" data-test="create-team-static">
          {{ teamName }}
        </div>
        <p class="pc-field__help">{{ $t("team.products.fieldTeamHelp") }}</p>
      </div>
      <div class="pc-actions">
        <Button
          variant="primary"
          size="sm"
          :disabled="!draftName.trim()"
          data-test="create-next"
          @click="step = 2"
        >
          {{ $t("team.products.chooseFirstOwner") }}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          data-test="create-cancel"
          @click="goToList"
        >
          {{ $t("team.products.cancel") }}
        </Button>
      </div>
    </section>

    <!-- 第 2 步：First Owner（C2） -->
    <template v-else-if="step === 2">
      <div class="pc-field">
        <span class="pc-field__label">{{
          $t("team.products.fieldFirstOwner")
        }}</span>
        <Select
          v-model="draftOwnerId"
          :options="ownerOptions"
          :placeholder="$t('team.products.ownerPlaceholder')"
          :aria-label="$t('team.products.fieldFirstOwner')"
          data-test="create-owner-select"
        />
        <p class="pc-field__help">{{ $t("team.products.fieldOwnerHelp") }}</p>
      </div>

      <section class="pc-card">
        <h2 class="pc-card__title">{{ $t("team.products.eligibleTitle") }}</h2>
        <p
          v-for="member in eligibleOwners"
          :key="member.principal_id"
          class="pc-eligible"
          :class="{ 'is-selected': member.principal_id === draftOwnerId }"
          :data-test="`create-candidate-${member.principal_id}`"
          role="button"
          tabindex="0"
          @click="draftOwnerId = member.principal_id"
          @keyup.enter="draftOwnerId = member.principal_id"
        >
          <span class="pc-eligible__name">{{ member.display_name }}</span>
          <span>{{
            member.role === "owner"
              ? $t("team.members.identityOwner")
              : $t("team.members.identityMember")
          }}</span>
          <Badge
            :variant="
              member.principal_id === draftOwnerId ? 'neutral' : undefined
            "
          >
            {{
              member.principal_id === draftOwnerId
                ? $t("team.products.candidateSelected")
                : $t("team.products.candidateAvailable")
            }}
          </Badge>
        </p>
      </section>

      <div class="pc-actions">
        <Button
          variant="primary"
          size="sm"
          :disabled="!selectedOwner"
          data-test="create-review"
          @click="step = 3"
        >
          {{ $t("team.products.reviewProduct") }}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          data-test="create-back-1"
          @click="step = 1"
        >
          {{ $t("team.products.back") }}
        </Button>
      </div>

      <Alert variant="warning" :title="$t('team.products.ownerMandatoryTitle')">
        {{ $t("team.products.ownerMandatoryDetail") }}
      </Alert>
    </template>

    <!-- 第 3 步：Review（C3） -->
    <template v-else>
      <section class="pc-card">
        <h2 class="pc-card__title">{{ draftName }}</h2>
        <div class="pc-review" data-testid="create-review-details">
          <div class="pc-review__row">
            <span class="pc-review__field">{{
              $t("team.products.reviewKey")
            }}</span>
            <span>{{ keyPreview || "—" }}</span>
          </div>
          <div class="pc-review__row">
            <span class="pc-review__field">{{
              $t("team.products.reviewTeam")
            }}</span>
            <span>{{ teamName }}</span>
          </div>
          <div class="pc-review__row">
            <span class="pc-review__field">{{
              $t("team.products.reviewOwner")
            }}</span>
            <span>{{ selectedOwner?.display_name ?? "—" }}</span>
          </div>
          <div class="pc-review__row">
            <span class="pc-review__field">{{
              $t("team.products.reviewCreator")
            }}</span>
            <span>{{
              $t("team.products.reviewCreatorValue", { name: selfName })
            }}</span>
          </div>
        </div>
      </section>

      <div class="pc-actions">
        <Button
          variant="primary"
          size="sm"
          :loading="submitting"
          data-test="create-submit"
          @click="submitCreate"
        >
          {{ $t("team.products.create") }}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          :disabled="submitting"
          data-test="create-back-2"
          @click="step = 2"
        >
          {{ $t("team.products.back") }}
        </Button>
      </div>

      <Alert variant="info" :title="$t('team.products.onlyFirstOwnerTitle')">
        {{ $t("team.products.onlyFirstOwnerDetail") }}
      </Alert>
    </template>

    <p class="pc-foot">
      {{
        $t("team.products.creatorFootnote", {
          name: selfName,
          role: $t("team.products.identityTeamOwner")
        })
      }}
    </p>
  </PageFrame>
</template>

<style scoped>
.pc-steps {
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

.pc-steps__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pc-steps__item.is-current {
  color: var(--text-primary);
}

.pc-steps__num {
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

.pc-steps__item.is-current .pc-steps__num {
  border-color: var(--ws-accent);
  color: var(--ws-accent);
}

.pc-steps__arrow {
  color: var(--text-tertiary);
}

.pc-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.pc-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-width: 44rem;
}

.pc-card__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.pc-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.pc-field__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.pc-field__static {
  display: flex;
  align-items: center;
  padding: 0 var(--space-3);
  height: 2.25rem;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-tertiary);
}

.pc-field__help {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.pc-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pc-eligible {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
  cursor: pointer;
}

.pc-eligible.is-selected {
  border-color: var(--ws-accent);
}

.pc-eligible__name {
  min-width: 10rem;
}

.pc-review {
  display: flex;
  flex-direction: column;
}

.pc-review__row {
  display: flex;
  align-items: baseline;
  gap: var(--space-4);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pc-review__row:last-child {
  border-bottom: none;
}

.pc-review__field {
  min-width: 16rem;
  color: var(--text-secondary);
}

.pc-foot {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
