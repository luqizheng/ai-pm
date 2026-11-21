<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { HttpError } from "@digua/http";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  fetchProduct,
  listProductLinks,
  listProducts,
  type ProductLink,
  type ProductProfile
} from "@/api/products";
import { listProjects } from "@/api/pm";
import type { Project } from "@/api/types";
import { useTeamStore } from "@/stores/team";
import TeamRestrictedState from "@/views/team/TeamRestrictedState.vue";
import { lifecyclePresentation } from "./product-display";

/**
 * 关联详情页（P4-ORG UX 修订 / Figma A2「Association details」）。
 *
 * Association 卡四行：Product / Owning Team / Project / Other associated
 * Product（同项目关联的其他产品）。Association only 边界横幅；Open
 * Project（项目可见时）+ Back to associations。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const restricted = ref(false);
const profile = ref<ProductProfile | null>(null);
const link = ref<ProductLink | null>(null);
const projects = ref<Project[]>([]);
const project = ref<Project | null>(null);
const otherKeys = ref<string[]>([]);

const productId = computed(() => String(route.params.productId));
const linkId = computed(() => String(route.params.linkId));
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const lifecycle = computed(() =>
  profile.value ? lifecyclePresentation(profile.value.status) : null
);

async function reload(): Promise<void> {
  if (!productId.value || !linkId.value) return;
  loading.value = true;
  error.value = null;
  restricted.value = false;
  try {
    const [profileRow, linkRows] = await Promise.all([
      fetchProduct(productId.value),
      listProductLinks(productId.value)
    ]);
    profile.value = profileRow;
    link.value = linkRows.find(row => row.id === linkId.value) ?? null;
    if (!link.value) {
      restricted.value = true;
      return;
    }
    projects.value = (await listProjects({ page: 1, size: 200 })).items ?? [];
    project.value =
      projects.value.find(item => item.id === link.value?.project_id) ?? null;
    // Other associated Product：同项目的其他产品（仅可读产品）。
    const teamProducts = await listProducts(profileRow.team_id);
    const keys: string[] = [];
    await Promise.all(
      teamProducts
        .filter(item => item.id !== productId.value)
        .map(async item => {
          try {
            const itemLinks = await listProductLinks(item.id);
            if (
              itemLinks.some(row => row.project_id === link.value?.project_id)
            ) {
              keys.push(item.key);
            }
          } catch {
            // 不可读产品如实跳过。
          }
        })
    );
    otherKeys.value = keys;
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

function openProject(): void {
  if (!link.value) return;
  void router.push({
    path: "/project/command-center",
    query: { project_id: link.value.project_id }
  });
}

function back(): void {
  void router.push(`/settings/team/products/${productId.value}/projects`);
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
        label: profile?.name ?? $t('team.products.tabProjects'),
        to: `/settings/team/products/${productId}/projects`
      },
      { label: $t('team.products.associationDetails') }
    ]"
    :title="$t('team.products.associationDetails')"
  >
    <TeamRestrictedState v-if="restricted" />

    <template v-else-if="profile && link && lifecycle">
      <p v-if="error" class="pd-error">{{ error }}</p>

      <section class="pd-card" data-testid="association-details">
        <h2 class="pd-card__title">
          {{
            $t("team.products.associationCardTitle", {
              key: profile.key,
              project: project?.name ?? `#${link.project_id.slice(0, 8)}`
            })
          }}
        </h2>
        <div class="pd-row">
          <span class="pd-row__field">{{
            $t("team.products.rowProduct")
          }}</span>
          <span>
            {{
              $t("team.products.rowProductValue", {
                name: profile.name,
                key: profile.key,
                lifecycle: $t(lifecycle.labelKey)
              })
            }}
          </span>
        </div>
        <div class="pd-row">
          <span class="pd-row__field">{{ $t("team.products.rowTeam") }}</span>
          <span>{{ teamName }}</span>
        </div>
        <div class="pd-row">
          <span class="pd-row__field">{{
            $t("team.products.rowProject")
          }}</span>
          <span>
            {{
              project
                ? $t("team.products.rowProjectValue", {
                    name: project.name,
                    status: project.status
                  })
                : `#${link.project_id.slice(0, 8)}`
            }}
          </span>
        </div>
        <div class="pd-row">
          <span class="pd-row__field">
            {{ $t("team.products.rowOtherProducts") }}
          </span>
          <span>{{ otherKeys.join(" · ") || "—" }}</span>
        </div>
      </section>

      <Alert variant="info" :title="$t('team.products.associationOnlyTitle')">
        {{ $t("team.products.associationOnlyDetail") }}
      </Alert>

      <p class="pd-foot">{{ $t("team.products.associationFootnote") }}</p>

      <div class="pd-actions">
        <Button
          v-if="project"
          variant="primary"
          size="sm"
          data-test="association-open-project"
          @click="openProject"
        >
          {{ $t("team.products.openProject") }}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          data-test="association-back"
          @click="back"
        >
          {{ $t("team.products.backToAssociations") }}
        </Button>
      </div>
    </template>
  </PageFrame>
</template>

<style scoped>
.pd-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.pd-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.pd-card__title {
  margin: 0 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.pd-row {
  display: flex;
  align-items: baseline;
  gap: var(--space-4);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pd-row:last-child {
  border-bottom: none;
}

.pd-row__field {
  min-width: 14rem;
  color: var(--text-secondary);
}

.pd-foot {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.pd-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
</style>
