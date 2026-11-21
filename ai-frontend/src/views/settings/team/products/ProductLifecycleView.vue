<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { HttpError } from "@digua/http";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmationDialog } from "@/components/ui/confirmation-dialog";
import {
  fetchProduct,
  listProductLinks,
  listProductMembers,
  setProductLifecycle,
  type ProductProfile
} from "@/api/products";
import { listProjects } from "@/api/pm";
import type { Project } from "@/api/types";
import { useTeamStore } from "@/stores/team";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import TeamRestrictedState from "@/views/team/TeamRestrictedState.vue";
import { lifecyclePresentation } from "./product-display";

/**
 * 产品生命周期页（P4-ORG UX 修订 / Figma L1「Lifecycle active」与
 * L2「abandoned」态）。
 *
 * Lifecycle decision 卡：Active → Danger「Abandon Product」（D-abandon
 * 确认弹窗）；archived →「Restart Product」（D-restart）。无级联警告横幅
 * + 关联执行上下文卡（Project/Project state——Agent Run 列后端无轻量读
 * 面，本版省略）。非成员 → 受限态。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const teamStore = useTeamStore();
const toast = useToast();

const loading = ref(true);
const error = ref<string | null>(null);
const restricted = ref(false);
const profile = ref<ProductProfile | null>(null);
const links = ref<Array<{ id: string; project_id: string }>>([]);
const projects = ref<Project[]>([]);
const submitting = ref(false);

/** 打开的确认弹窗：null | abandon | restart */
const confirmAction = ref<"abandon" | "restart" | null>(null);

const productId = computed(() => String(route.params.productId));
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const lifecycle = computed(() =>
  profile.value ? lifecyclePresentation(profile.value.status) : null
);
const isArchived = computed(() => profile.value?.status === "archived");

const selfId = computed(() => teamStore.context?.principal_id ?? "");
const viewerIsOwner = ref(false);

interface ContextRow {
  projectId: string;
  project: Project | null;
}

const contextRows = computed<ContextRow[]>(() =>
  links.value.map(link => ({
    projectId: link.project_id,
    project:
      projects.value.find(project => project.id === link.project_id) ?? null
  }))
);

async function reload(): Promise<void> {
  if (!productId.value) return;
  loading.value = true;
  error.value = null;
  restricted.value = false;
  try {
    const [profileRow, linkRows] = await Promise.all([
      fetchProduct(productId.value),
      listProductLinks(productId.value)
    ]);
    profile.value = profileRow;
    links.value = linkRows;
    // 成员行校验调用者身份（You · Product Owner 徽标）。
    try {
      const memberRows = await listProductMembers(productId.value);
      viewerIsOwner.value = memberRows.some(
        m =>
          m.principal_id === selfId.value && m.role === "owner" && m.is_active
      );
    } catch {
      viewerIsOwner.value = false;
    }
    projects.value = (await listProjects({ page: 1, size: 200 })).items ?? [];
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

async function applyLifecycle(): Promise<void> {
  const action = confirmAction.value;
  if (!action || submitting.value) return;
  submitting.value = true;
  error.value = null;
  try {
    profile.value = await setProductLifecycle(productId.value, action);
    toast.toast({
      title:
        action === "abandon"
          ? t("team.products.abandonedToast")
          : t("team.products.restartedToast")
    });
    confirmAction.value = null;
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
    confirmAction.value = null;
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
        label: profile?.name ?? $t('team.products.lifecycleTitle'),
        to: `/settings/team/products/${productId}`
      },
      { label: $t('team.products.lifecycleTitle') }
    ]"
    :title="$t('team.products.lifecycleTitle')"
  >
    <TeamRestrictedState v-if="restricted" />

    <template v-else-if="profile && lifecycle">
      <p v-if="error" class="pl-error">{{ error }}</p>

      <!-- 徽标行（L1 顶部） -->
      <div class="pl-badges" data-testid="lifecycle-badges">
        <Badge variant="neutral">
          {{
            viewerIsOwner
              ? $t("team.products.badgeYouOwner")
              : $t("team.products.badgeYouMember")
          }}
        </Badge>
        <Badge :variant="lifecycle.variant">
          {{ $t(lifecycle.labelKey) }}
        </Badge>
      </div>

      <!-- Lifecycle decision 卡 -->
      <section class="pl-card" data-testid="lifecycle-decision">
        <h2 class="pl-card__title">{{ profile.name }} · {{ profile.key }}</h2>
        <p class="pl-card__desc">
          {{
            isArchived
              ? $t("team.products.lifecycleArchivedDesc")
              : $t("team.products.lifecycleActiveDesc")
          }}
        </p>
        <div class="pl-actions">
          <Button
            v-if="!isArchived"
            variant="danger"
            size="sm"
            data-test="lifecycle-abandon"
            @click="confirmAction = 'abandon'"
          >
            {{ $t("team.products.abandonAction") }}
          </Button>
          <Button
            v-else
            variant="secondary"
            size="sm"
            data-test="lifecycle-restart"
            @click="confirmAction = 'restart'"
          >
            {{ $t("team.products.restartAction") }}
          </Button>
        </div>
      </section>

      <!-- 无级联警告（L1 Warning 横幅） -->
      <Alert variant="warning" :title="$t('team.products.noCascadingTitle')">
        {{ $t("team.products.noCascadingDetail") }}
      </Alert>

      <!-- 关联执行上下文卡（L1；Agent Run 列数据面未设，仅 Project/State） -->
      <section class="pl-card" data-testid="lifecycle-context">
        <h2 class="pl-card__title">
          {{ $t("team.products.contextTitle") }}
        </h2>
        <p v-if="contextRows.length === 0" class="pl-card__empty">
          {{ $t("team.products.associationsEmpty") }}
        </p>
        <div v-for="row in contextRows" :key="row.projectId" class="pl-context">
          <span class="pl-context__name">
            {{ row.project?.name ?? `#${row.projectId.slice(0, 8)}` }}
          </span>
          <Badge v-if="row.project" variant="neutral">
            {{ row.project.status }}
          </Badge>
        </div>
      </section>

      <p class="pl-foot">{{ $t("team.products.lifecycleFootnote") }}</p>

      <!-- D-abandon / D-restart 确认弹窗（DS/Overlays/ConfirmationDialog） -->
      <ConfirmationDialog
        :open="confirmAction === 'abandon'"
        :title="$t('team.products.abandonConfirmTitle')"
        :description="$t('team.products.abandonConfirmDetail')"
        :confirm-label="$t('team.products.abandonAction')"
        :cancel-label="$t('team.products.cancel')"
        variant="danger"
        :checkbox-label="$t('team.products.reviewedCheckbox')"
        :disabled="submitting"
        @update:open="
          value => {
            if (!value) confirmAction = null;
          }
        "
        @confirm="applyLifecycle"
      />
      <ConfirmationDialog
        :open="confirmAction === 'restart'"
        :title="$t('team.products.restartConfirmTitle')"
        :description="$t('team.products.restartConfirmDetail')"
        :confirm-label="$t('team.products.restartAction')"
        :cancel-label="$t('team.products.cancel')"
        :checkbox-label="$t('team.products.reviewedCheckbox')"
        :disabled="submitting"
        @update:open="
          value => {
            if (!value) confirmAction = null;
          }
        "
        @confirm="applyLifecycle"
      />
    </template>
  </PageFrame>
</template>

<style scoped>
.pl-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.pl-badges {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.pl-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.pl-card__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.pl-card__desc {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.pl-card__empty {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.pl-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pl-context {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.pl-context:last-child {
  border-bottom: none;
}

.pl-context__name {
  flex: 1;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pl-foot {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
