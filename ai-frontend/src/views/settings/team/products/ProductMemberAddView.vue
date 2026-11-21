<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import {
  addProductMember,
  fetchProduct,
  listProductMembers,
  type ProductProfile
} from "@/api/products";
import { listTeamMembers, type TeamMember } from "@/api/team";
import { useTeamStore } from "@/stores/team";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";

/**
 * 添加产品成员页（P4-ORG UX 修订 / Figma M2「Add Product Member」）。
 *
 * 候选 = 本团队成员（同租户边界由团队归属保证；租户级全量目录 V1 不设，
 * 后端服务端仍按同租户强制）；身份固定为普通 Product Member（设计稿
 * disabled 字段：无其他人类产品角色可选）；Result preview 三列说明变更
 * 边界（新关系只落在 Product，项目访问与团队归属不变）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const teamStore = useTeamStore();
const toast = useToast();

const loading = ref(true);
const error = ref<string | null>(null);
const profile = ref<ProductProfile | null>(null);
const existing = ref<Array<{ principal_id: string }>>([]);
const candidates = ref<TeamMember[]>([]);
const candidateId = ref("");
const submitting = ref(false);

const productId = computed(() => String(route.params.productId));
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const productName = computed(() => profile.value?.name ?? "");

const candidateOptions = computed(() =>
  candidates.value
    .filter(member => member.is_active)
    .filter(
      member =>
        !existing.value.some(row => row.principal_id === member.principal_id)
    )
    .map(member => ({ value: member.principal_id, label: member.display_name }))
);

const selectedCandidate = computed(() =>
  candidates.value.find(member => member.principal_id === candidateId.value)
);

async function reload(): Promise<void> {
  if (!productId.value) return;
  loading.value = true;
  error.value = null;
  try {
    const [profileRow, memberRows, teamMemberRows] = await Promise.all([
      fetchProduct(productId.value),
      listProductMembers(productId.value),
      teamStore.context?.team?.id
        ? listTeamMembers(teamStore.context.team.id)
        : Promise.resolve([])
    ]);
    profile.value = profileRow;
    existing.value = memberRows;
    candidates.value = teamMemberRows;
  } catch (err) {
    error.value =
      err instanceof Error ? err.message : t("team.products.loadFailed");
  } finally {
    loading.value = false;
  }
}

function cancel(): void {
  void router.push(`/settings/team/products/${productId.value}/members`);
}

async function submitAdd(): Promise<void> {
  if (!candidateId.value || submitting.value) return;
  submitting.value = true;
  error.value = null;
  try {
    await addProductMember(productId.value, candidateId.value);
    toast.toast({
      title: t("team.products.memberAddedToast", {
        name: selectedCandidate.value?.display_name ?? ""
      })
    });
    void router.push(`/settings/team/products/${productId.value}/members`);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    submitting.value = false;
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
      {
        label: productName || $t('team.products.tabMembers'),
        to: `/settings/team/products/${productId}/members`
      },
      { label: $t('team.products.addMember') }
    ]"
    :title="$t('team.products.addMember')"
  >
    <Alert variant="info" :title="$t('team.products.addOrdinaryTitle')">
      {{ $t("team.products.addOrdinaryDetail") }}
    </Alert>

    <p v-if="error" class="pa-error">{{ error }}</p>

    <div class="pa-field">
      <span class="pa-field__label">{{
        $t("team.products.fieldPrincipal")
      }}</span>
      <Select
        v-model="candidateId"
        :options="candidateOptions"
        :placeholder="$t('team.products.principalPlaceholder')"
        :aria-label="$t('team.products.fieldPrincipal')"
        data-test="add-principal-select"
      />
      <p class="pa-field__help">{{ $t("team.products.fieldPrincipalHelp") }}</p>
    </div>

    <div class="pa-field">
      <span class="pa-field__label">{{
        $t("team.products.fieldIdentity")
      }}</span>
      <div class="pa-field__static" data-test="add-identity-static">
        {{ $t("team.products.identityProductMember") }}
      </div>
      <p class="pa-field__help">{{ $t("team.products.fieldIdentityHelp") }}</p>
    </div>

    <div class="pa-actions">
      <Button
        variant="primary"
        size="sm"
        :disabled="!candidateId"
        :loading="submitting"
        data-test="add-submit"
        @click="submitAdd"
      >
        {{ $t("team.products.addMember") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        :disabled="submitting"
        data-test="add-cancel"
        @click="cancel"
      >
        {{ $t("team.products.cancel") }}
      </Button>
    </div>

    <!-- Result preview（M2 底部三列） -->
    <section class="pa-card" data-testid="add-result-preview">
      <h2 class="pa-card__title">{{ $t("team.products.previewTitle") }}</h2>
      <div class="pa-preview">
        <span class="pa-preview__head">{{
          $t("team.products.previewPerson")
        }}</span>
        <span class="pa-preview__head">{{
          $t("team.products.previewNew")
        }}</span>
        <span class="pa-preview__head">{{
          $t("team.products.previewUnchanged")
        }}</span>
      </div>
      <div class="pa-preview">
        <span>{{ selectedCandidate?.display_name ?? "—" }}</span>
        <span>
          {{
            $t("team.products.previewNewValue", {
              key: profile?.key ?? "—"
            })
          }}
        </span>
        <span>{{ $t("team.products.previewUnchangedValue") }}</span>
      </div>
    </section>
  </PageFrame>
</template>

<style scoped>
.pa-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.pa-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  max-width: 42rem;
}

.pa-field__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.pa-field__static {
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

.pa-field__help {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.pa-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pa-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.pa-card__title {
  margin: 0 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.pa-preview {
  display: grid;
  grid-template-columns: 1fr 1.2fr 1.6fr;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pa-preview__head {
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  color: var(--text-tertiary);
}
</style>
