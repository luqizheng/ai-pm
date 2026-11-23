<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import PageFrame from "@/components/shell/PageFrame.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import type { BadgeVariant } from "@/components/ui/badge";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { listProjectProducts } from "@/api/products";
import type { ProjectProduct } from "@/api/products";
import { describeError } from "@/lib/errors";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";

/**
 * PA-07 产品页（Figma 179:3603 E 系列）：项目关联产品列表。
 * 四态生命周期门禁徽标（Normal/Suspended/Resuming/Archived），无产品时
 * 诚实空态（产品↔项目的 Link 由产品 Owner 建立，本页只读呈现）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const scope = useScopeStore();

const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  if (scope.currentProjectId && scope.currentProjectId !== SCOPE_ALL) {
    return scope.currentProjectId;
  }
  return "";
});

const products = ref<ProjectProduct[]>([]);
const loading = ref(false);
const loadError = ref<string | null>(null);

const STATUS_TONES: Record<string, BadgeVariant> = {
  normal: "success",
  resuming: "info",
  suspended: "warning",
  archived: "neutral"
};

function statusToneClass(status: string): BadgeVariant {
  return STATUS_TONES[status] ?? "neutral";
}

async function load() {
  if (!projectId.value) {
    products.value = [];
    return;
  }
  loading.value = true;
  loadError.value = null;
  try {
    products.value = await listProjectProducts(projectId.value);
  } catch (error) {
    loadError.value = describeError(error);
    products.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(() => void load());
watch(projectId, () => void load());

function openProduct(item: ProjectProduct) {
  void router.push(`/project/products/${item.product.id}`);
}
</script>

<template>
  <PageFrame
    :title="t('pa07.products.title')"
    :description="t('pa07.products.subtitle')"
  >
    <p v-if="loadError" class="pr-error">{{ loadError }}</p>
    <p v-else-if="loading" class="pr-note">{{ t("common.loading") }}</p>
    <EmptyState
      v-else-if="products.length === 0"
      :title="t('pa07.panel.relatedEmpty')"
      data-test="products-empty"
    />

    <ul v-else class="pr-list" data-testid="products-list">
      <li v-for="item in products" :key="item.link_id">
        <button
          class="pr-card"
          data-test="product-card"
          @click="openProduct(item)"
        >
          <span class="pr-card__name">{{ item.product.name }}</span>
          <span class="pr-card__key">{{ item.product.key }}</span>
          <span
            class="pr-card__status"
            :class="{
              'is-restricted':
                item.product.status === 'suspended' ||
                item.product.status === 'archived'
            }"
          >
            <Badge :variant="statusToneClass(item.product.status)">
              {{
                t(
                  `pa07.products.status${item.product.status.charAt(0).toUpperCase() + item.product.status.slice(1)}`
                )
              }}
            </Badge>
          </span>
          <span class="pr-card__meta">
            {{ t("pa07.products.taskCountLabel", { count: item.task_count }) }}
          </span>
        </button>
      </li>
    </ul>
  </PageFrame>
</template>

<style scoped>
.pr-error {
  margin: 0;
  color: var(--ws-color-danger-text);
}

.pr-note {
  margin: 0;
  color: var(--ws-text-secondary);
}

.pr-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}

.pr-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-base);
  cursor: pointer;
  text-align: left;
}

.pr-card__name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.pr-card__key {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-disabled);
}

.pr-card__status {
  display: inline-flex;
}

.pr-card__meta {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}
</style>
