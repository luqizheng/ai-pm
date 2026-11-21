<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { HttpError } from "@digua/http";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  fetchProduct,
  listProductLinks,
  listProducts,
  type ProductProfile
} from "@/api/products";
import { listProjects } from "@/api/pm";
import type { Project } from "@/api/types";
import { useTeamStore } from "@/stores/team";
import TeamRestrictedState from "@/views/team/TeamRestrictedState.vue";
import { lifecyclePresentation } from "./product-display";

/**
 * 关联项目页（P4-ORG UX 修订 / Figma A1「Associated Projects」；A0 空态
 * 同页空列表）。
 *
 * 列 = Project · State · Product links（同项目关联的其他产品，经全团队
 * 产品 Link 枚举聚合，不可读的产品如实省略）· Access / action（可见项目
 * → View association；不可见 → Restricted 文本）。
 * **设计稿明确省略 Link / unlink 控件**（授权未经显式批准）——本页只读。
 */
const { t } = useI18n();
const route = useRoute();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const restricted = ref(false);
const profile = ref<ProductProfile | null>(null);
const links = ref<Array<{ id: string; project_id: string }>>([]);
const projects = ref<Project[]>([]);
/** project_id → 其他产品 key 集合（不含当前产品） */
const projectProductKeys = ref<Record<string, string[]>>({});

const productId = computed(() => String(route.params.productId));
const teamId = computed(() => teamStore.context?.team?.id ?? "");
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const lifecycle = computed(() =>
  profile.value ? lifecyclePresentation(profile.value.status) : null
);

interface AssociationRow {
  linkId: string;
  projectId: string;
  project: Project | null;
  otherKeys: string[];
}

const rows = computed<AssociationRow[]>(() =>
  links.value.map(link => ({
    linkId: link.id,
    projectId: link.project_id,
    project:
      projects.value.find(project => project.id === link.project_id) ?? null,
    otherKeys: (projectProductKeys.value[link.project_id] ?? []).filter(
      key => key !== profile.value?.key
    )
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
    projects.value = (await listProjects({ page: 1, size: 200 })).items ?? [];

    // Product links 列：枚举本团队各产品的 Link（仅可读产品；404 如实跳过）。
    const teamProducts = await listProducts(teamId.value);
    const map: Record<string, string[]> = {};
    await Promise.all(
      teamProducts.map(async product => {
        try {
          const productLinks = await listProductLinks(product.id);
          for (const link of productLinks) {
            (map[link.project_id] ??= []).push(product.key);
          }
        } catch {
          // 非 Product 成员读不到其他产品的 Link 表：不冒充。
        }
      })
    );
    projectProductKeys.value = map;
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
        to: `/settings/team/products/${productId}`
      },
      { label: $t('team.products.tabProjects') }
    ]"
    :title="profile?.name ?? ''"
  >
    <TeamRestrictedState v-if="restricted" />

    <template v-else-if="profile && lifecycle">
      <p v-if="error" class="pa1-error">{{ error }}</p>

      <!-- 顶部徽标行（A1） -->
      <div class="pp-head" data-testid="product-projects-head">
        <Badge variant="neutral">
          {{ profile.key }} · {{ $t(lifecycle.labelKey) }}
        </Badge>
        <span class="pp-head__note">
          {{
            $t("team.products.associationsCountLine", {
              count: rows.length
            })
          }}
        </span>
      </div>

      <!-- Associated Projects 卡 -->
      <section class="pp-card" data-testid="product-projects-card">
        <h2 class="pp-card__title">
          {{ $t("team.products.associatedPlainTitle") }}
        </h2>
        <p v-if="rows.length === 0" class="pp-card__empty">
          {{ $t("team.products.associationsEmpty") }}
        </p>
        <template v-else>
          <div class="pp-table__head">
            <span>{{ $t("team.products.colProject") }}</span>
            <span>{{ $t("team.products.colState") }}</span>
            <span>{{ $t("team.products.colProductLinks") }}</span>
            <span>{{ $t("team.products.colAccessAction") }}</span>
          </div>
          <div
            v-for="row in rows"
            :key="row.linkId"
            class="pp-table__row"
            :data-testid="`association-row-${row.linkId}`"
          >
            <span>{{
              row.project?.name ?? `#${row.projectId.slice(0, 8)}`
            }}</span>
            <span>
              <Badge v-if="row.project" variant="neutral">
                {{ row.project.status }}
              </Badge>
              <template v-else>—</template>
            </span>
            <span>{{ row.otherKeys.join(" · ") || "—" }}</span>
            <span>
              <Button
                v-if="row.project"
                variant="secondary"
                size="sm"
                :data-test="`association-view-${row.linkId}`"
                @click="
                  $router.push(
                    `/settings/team/products/${productId}/projects/${row.linkId}`
                  )
                "
              >
                {{ $t("team.products.viewAssociation") }}
              </Button>
              <span v-else>{{ $t("team.products.restricted") }}</span>
            </span>
          </div>
        </template>
      </section>

      <!-- 多对多说明（A1 Info 横幅） -->
      <Alert variant="info" :title="$t('team.products.m2mTitle')">
        {{ $t("team.products.m2mDetail") }}
      </Alert>

      <!-- A1 页脚：只读声明 -->
      <p class="pp-foot">
        {{ $t("team.products.readonlyLine1") }}
        <br />
        {{ $t("team.products.readonlyLine2") }}
      </p>
    </template>
  </PageFrame>
</template>

<style scoped>
.pa1-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.pp-head {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.pp-head__note {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pp-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.pp-card__title {
  margin: 0 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.pp-card__empty {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.pp-table__head,
.pp-table__row {
  display: grid;
  grid-template-columns: 1.6fr 0.8fr 1.2fr 1.4fr;
  gap: var(--space-3);
  align-items: center;
}

.pp-table__head {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  color: var(--text-tertiary);
  padding-bottom: var(--space-2);
  border-bottom: 1px solid var(--border-subtle);
}

.pp-table__row {
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pp-table__row:last-child {
  border-bottom: none;
}

.pp-foot {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}
</style>
