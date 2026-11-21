<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { HttpError } from "@digua/http";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Avatar } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmationDialog } from "@/components/ui/confirmation-dialog";
import { Select } from "@/components/ui/select";
import {
  appointProductOwner,
  fetchProduct,
  listProductMembers,
  removeProductMember,
  type ProductMember,
  type ProductProfile
} from "@/api/products";
import { listTeamMembers } from "@/api/team";
import { useTeamStore } from "@/stores/team";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import TeamRestrictedState from "@/views/team/TeamRestrictedState.vue";
import ProductTabs from "./ProductTabs.vue";
import { formatMembershipDate } from "@/views/team/format-date";

/**
 * 产品成员页（P4-ORG UX 修订 / Figma M1「Product members」）。
 *
 * 结构：Owners 卡（只读——「No Owner appointment or demotion action in
 * this view」；零 Owner 时 Team Owner 可恢复性任命，M5）+ Members 卡
 * （普通成员行含 Remove，经 D-remove 确认弹窗；任何 Product 成员平权可
 * 加删普通成员）。M4 末位 Owner 保护由 Owners/Members 分卡 + 服务端强制
 * 承载，不设独立降级入口。非 Product 成员 → 受限态。
 */
const { t, locale } = useI18n();
const route = useRoute();
const teamStore = useTeamStore();
const toast = useToast();

const loading = ref(true);
const error = ref<string | null>(null);
const restricted = ref(false);
const profile = ref<ProductProfile | null>(null);
const members = ref<ProductMember[]>([]);
const teamCandidates = ref<
  Array<{ principal_id: string; display_name: string }>
>([]);
const busyId = ref<string | null>(null);

const productId = computed(() => String(route.params.productId));
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const selfId = computed(() => teamStore.context?.principal_id ?? "");
const isTeamOwner = computed(() => teamStore.teamRole === "owner");

const owners = computed(() => members.value.filter(m => m.role === "owner"));
const ordinaryMembers = computed(() =>
  members.value.filter(m => m.role !== "owner")
);

/** 恢复性任命（M5）：零 Owner 且调用者=Team Owner（服务端同规则强制） */
const needsOwnerRestore = computed(
  () => owners.value.filter(o => o.is_active).length === 0
);
const restoreCandidateId = ref("");
const restoreCandidates = computed(() =>
  ordinaryMembers.value.filter(m => m.is_active)
);

/** 普通成员行操作 = Remove（Owner 行无操作；M4 保护语义由分卡承载） */
const removeTarget = ref<ProductMember | null>(null);

async function reload(): Promise<void> {
  if (!productId.value) return;
  loading.value = true;
  error.value = null;
  restricted.value = false;
  try {
    const [profileRow, memberRows] = await Promise.all([
      fetchProduct(productId.value),
      listProductMembers(productId.value)
    ]);
    profile.value = profileRow;
    members.value = memberRows;
    // 候选人 = 本团队成员（同租户边界由团队归属保证；租户级目录 V1 不设）。
    if (teamId.value) {
      teamCandidates.value = await listTeamMembers(teamId.value);
    }
  } catch (err) {
    if (
      err instanceof HttpError &&
      (err.status === 403 || err.status === 404)
    ) {
      restricted.value = true;
    } else {
      error.value =
        err instanceof Error ? err.message : t("team.products.loadFailed");
    }
  } finally {
    loading.value = false;
  }
}

const teamId = computed(() => teamStore.context?.team?.id ?? "");

async function confirmRemove(): Promise<void> {
  const target = removeTarget.value;
  if (!target) return;
  busyId.value = target.principal_id;
  try {
    await removeProductMember(productId.value, target.principal_id);
    toast.toast({
      title: t("team.products.memberRemovedToast", {
        name: target.display_name
      })
    });
    removeTarget.value = null;
    await reload();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busyId.value = null;
  }
}

async function restoreOwner(): Promise<void> {
  if (!restoreCandidateId.value) return;
  busyId.value = restoreCandidateId.value;
  try {
    await appointProductOwner(productId.value, restoreCandidateId.value);
    toast.toast({ title: t("team.products.ownerRestoredToast") });
    restoreCandidateId.value = "";
    await reload();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busyId.value = null;
  }
}

onMounted(async () => {
  await teamStore.load();
  await reload();
});
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: $t('team.tabs.domain') },
      { label: teamName, to: '/dashboard' },
      { label: $t('team.products.title'), to: '/settings/team/products' },
      { label: profile?.name ?? $t('team.products.tabMembers') }
    ]"
    :title="profile?.name ?? ''"
  >
    <TeamRestrictedState v-if="restricted" />

    <template v-else>
      <p v-if="error" class="pm-error">{{ error }}</p>

      <ProductTabs :product-id="productId" />

      <!-- 顶部：身份徽标 + Add Product Member -->
      <div class="pm-head">
        <Badge variant="neutral">
          {{
            owners.some(o => o.principal_id === selfId)
              ? $t("team.products.badgeYouOwner")
              : $t("team.products.badgeYouMember")
          }}
        </Badge>
        <Button
          variant="primary"
          size="sm"
          data-test="members-add"
          @click="
            $router.push(`/settings/team/products/${productId}/members/add`)
          "
        >
          {{ $t("team.products.addMember") }}
        </Button>
      </div>

      <!-- Owners 卡（只读；零 Owner → Team Owner 恢复性任命） -->
      <section class="pm-card" data-testid="product-owners-card">
        <h2 class="pm-card__title">
          {{ $t("team.products.ownersTitle", { count: owners.length }) }}
        </h2>
        <div v-for="owner in owners" :key="owner.principal_id" class="pm-owner">
          <Avatar :name="owner.display_name" size="md" />
          <div class="pm-owner__body">
            <span class="pm-owner__name">{{ owner.display_name }}</span>
            <Badge variant="accent">
              {{ $t("team.products.badgeProductOwner") }}
            </Badge>
          </div>
          <div class="pm-owner__note">
            <p>{{ $t("team.products.ownerNoteMain") }}</p>
            <p class="pm-owner__note-sub">
              {{ $t("team.products.ownerNoteSub") }}
            </p>
          </div>
        </div>
        <p v-if="owners.length === 0" class="pm-card__empty">
          {{ $t("team.products.noOwnerNote") }}
        </p>
        <div v-if="needsOwnerRestore && isTeamOwner" class="pm-restore">
          <Select
            v-model="restoreCandidateId"
            :options="
              restoreCandidates.map(m => ({
                value: m.principal_id,
                label: m.display_name
              }))
            "
            :placeholder="$t('team.products.restorePlaceholder')"
            :aria-label="$t('team.products.restoreAction')"
            data-test="restore-owner-select"
          />
          <Button
            variant="primary"
            size="sm"
            :disabled="!restoreCandidateId"
            :loading="busyId === restoreCandidateId"
            data-test="restore-owner-confirm"
            @click="restoreOwner"
          >
            {{ $t("team.products.restoreAction") }}
          </Button>
        </div>
      </section>

      <!-- Members 卡 -->
      <section class="pm-card" data-testid="product-members-card">
        <h2 class="pm-card__title">
          {{
            $t("team.products.membersTitle", { count: ordinaryMembers.length })
          }}
        </h2>
        <div class="pm-table">
          <div class="pm-table__head">
            <span>{{ $t("team.products.colHumanPrincipal") }}</span>
            <span>{{ $t("team.products.colProductIdentity") }}</span>
            <span>{{ $t("team.members.colJoined") }}</span>
            <span>{{ $t("team.products.colStatusAction") }}</span>
          </div>
          <div
            v-for="member in ordinaryMembers"
            :key="member.principal_id"
            class="pm-table__row"
            :data-testid="`product-member-row-${member.principal_id}`"
          >
            <span>{{ member.display_name }}</span>
            <span>{{ $t("team.products.identityProductMember") }}</span>
            <span>{{ formatMembershipDate(member.created_at, locale) }}</span>
            <span>
              {{
                member.is_active
                  ? $t("team.members.statusActive")
                  : $t("team.members.statusInactive")
              }}
              <Button
                variant="secondary"
                size="sm"
                class="pm-remove"
                :data-test="`member-remove-${member.principal_id}`"
                @click="removeTarget = member"
              >
                {{ $t("team.products.removeAction") }}
              </Button>
            </span>
          </div>
          <p v-if="ordinaryMembers.length === 0" class="pm-card__empty">
            {{ $t("team.products.noMembersNote") }}
          </p>
        </div>
      </section>

      <!-- 平权说明（M1 Info 横幅） -->
      <Alert variant="info" :title="$t('team.products.sharedManagementTitle')">
        {{ $t("team.products.sharedManagementDetail") }}
      </Alert>

      <!-- D-remove 确认弹窗 -->
      <ConfirmationDialog
        :open="removeTarget !== null"
        :title="
          $t('team.products.removeConfirmTitle', {
            name: removeTarget?.display_name
          })
        "
        :description="$t('team.products.removeConfirmDetail')"
        :confirm-label="$t('team.products.removeAction')"
        :cancel-label="$t('team.products.cancel')"
        variant="danger"
        :checkbox-label="$t('team.products.reviewedCheckbox')"
        @update:open="
          value => {
            if (!value) removeTarget = null;
          }
        "
        @confirm="confirmRemove"
      />
    </template>
  </PageFrame>
</template>

<style scoped>
.pm-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.pm-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.pm-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.pm-card__title {
  margin: 0 0 var(--space-4);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.pm-card__empty {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.pm-owner {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-2) 0;
}

.pm-owner__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 14rem;
}

.pm-owner__name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.pm-owner__note {
  flex: 1;
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pm-owner__note p {
  margin: 0;
}

.pm-owner__note-sub {
  margin-top: var(--space-2);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.pm-restore {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-3);
}

.pm-table {
  display: flex;
  flex-direction: column;
}

.pm-table__head,
.pm-table__row {
  display: grid;
  grid-template-columns: 2fr 1.6fr 1.4fr 1.8fr;
  gap: var(--space-3);
  align-items: center;
}

.pm-table__head {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  color: var(--text-tertiary);
  padding-bottom: var(--space-2);
  border-bottom: 1px solid var(--border-subtle);
}

.pm-table__row {
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pm-table__row:last-child {
  border-bottom: none;
}

.pm-remove {
  margin-left: var(--space-3);
}
</style>
