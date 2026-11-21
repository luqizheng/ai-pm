<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { HttpError } from "@digua/http";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  fetchProduct,
  listProductLinks,
  listProductMembers,
  type ProductProfile
} from "@/api/products";
import { listProjects } from "@/api/pm";
import { fetchProductFeed, type FeedItem } from "@/api/feeds";
import type { Project } from "@/api/types";
import { useTeamStore } from "@/stores/team";
import TeamRestrictedState from "@/views/team/TeamRestrictedState.vue";
import { useRelativeTime } from "@/views/personal/useRelativeTime";
import ProductTabs from "./ProductTabs.vue";
import { lifecyclePresentation } from "./product-display";

/**
 * 产品概览页（P4-ORG UX 修订 / Figma O1「Product overview」；非成员 =
 * O2 只读受限态）。
 *
 * 徽标行 = [key] [Lifecycle] [You · Product 身份]；概览卡（名称/描述/
 * Owner·成员·关联计数）；Associated Projects·independently authorized
 * 卡（Your access = 项目是否在当前用户可见项目集，可见→Open Project，
 * 否则 Restricted 文本）。Lifecycle 不是页签，经「View lifecycle」进入。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const restricted = ref(false);
const profile = ref<ProductProfile | null>(null);
const members = ref<
  Array<{
    principal_id: string;
    role: string;
    display_name: string;
    is_active: boolean;
  }>
>([]);
const links = ref<Array<{ id: string; project_id: string }>>([]);
const projects = ref<Project[]>([]);
/** O1 页脚 Activity（PA-05-08 Product Feed 视图，前 5 条） */
const activity = ref<FeedItem[]>([]);

const productId = computed(() => String(route.params.productId));
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const { relative } = useRelativeTime();

const lifecycle = computed(() =>
  profile.value ? lifecyclePresentation(profile.value.status) : null
);

const activeOwner = computed(() =>
  members.value.find(member => member.role === "owner" && member.is_active)
);

const activeMemberCount = computed(
  () => members.value.filter(member => member.is_active).length
);

/** 身份徽标：Product Owner / Product Member（页面本身要求成员身份，非成员走受限态） */
const selfId = computed(() => teamStore.context?.principal_id ?? "");
const viewerMember = computed(() =>
  members.value.find(member => member.principal_id === selfId.value)
);
const identityLabel = computed(() => {
  if (!viewerMember.value) return null;
  return viewerMember.value.role === "owner"
    ? t("team.products.badgeYouOwner")
    : t("team.products.badgeYouMember");
});

interface AssociationRow {
  linkId: string;
  projectId: string;
  project: Project | null;
}

const associationRows = computed<AssociationRow[]>(() =>
  links.value.map(link => ({
    linkId: link.id,
    projectId: link.project_id,
    project:
      projects.value.find(project => project.id === link.project_id) ?? null
  }))
);

function openProject(projectId: string): void {
  // 项目工作区入口 = 项目总控台（同仪表盘项目入口口径）。
  void router.push({
    path: "/project/command-center",
    query: { project_id: projectId }
  });
}

async function reload(): Promise<void> {
  if (!productId.value) return;
  loading.value = true;
  error.value = null;
  restricted.value = false;
  try {
    const [profileRow, memberRows, linkRows] = await Promise.all([
      fetchProduct(productId.value),
      listProductMembers(productId.value),
      listProductLinks(productId.value)
    ]);
    profile.value = profileRow;
    members.value = memberRows;
    links.value = linkRows;
    // Your access 判定源 = 当前用户可见项目集（成员/Owner 语义由后端过滤）。
    projects.value = (await listProjects({ page: 1, size: 200 })).items ?? [];
    // O1 页脚 Activity = PA-05-08 Product Feed 视图（失败降级空态，不阻塞页面）。
    try {
      activity.value = await fetchProductFeed(productId.value, 5, 0);
    } catch {
      activity.value = [];
    }
  } catch (err) {
    if (
      err instanceof HttpError &&
      (err.status === 403 || err.status === 404)
    ) {
      // 非该 Product 成员（404）或无权（403）→ O2 只读受限态。
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
      { label: profile?.name ?? $t('team.products.title') }
    ]"
    :title="profile?.name ?? ''"
  >
    <TeamRestrictedState v-if="restricted" />

    <template v-else-if="profile && lifecycle">
      <p v-if="error" class="po-error">{{ error }}</p>

      <ProductTabs :product-id="productId" />

      <!-- 徽标行（O1 顶部） -->
      <div class="po-badges" data-testid="product-badges">
        <Badge variant="neutral">{{ profile.key }}</Badge>
        <Badge :variant="lifecycle.variant">
          {{ $t(lifecycle.labelKey) }}
        </Badge>
        <Badge v-if="identityLabel" variant="neutral">
          {{ identityLabel }}
        </Badge>
      </div>

      <!-- 业务概览卡 -->
      <section class="po-summary" data-testid="product-overview-summary">
        <h2 class="po-summary__title">{{ profile.name }}</h2>
        <p class="po-summary__desc">{{ profile.description || "—" }}</p>
        <div class="po-summary__meta" data-testid="product-overview-meta">
          <span>
            {{
              $t("team.products.metaOwner", {
                name: activeOwner?.display_name ?? "—"
              })
            }}
          </span>
          <span>
            {{ $t("team.products.metaMembers", { count: activeMemberCount }) }}
          </span>
          <span>
            {{
              $t("team.products.metaProjects", {
                count: associationRows.length
              })
            }}
          </span>
        </div>
      </section>

      <!-- 关联项目 · 独立授权 -->
      <section class="po-card" data-testid="product-overview-projects">
        <h2 class="po-card__title">
          {{ $t("team.products.associatedTitle") }}
        </h2>
        <p v-if="associationRows.length === 0" class="po-card__empty">
          {{ $t("team.products.associationsEmpty") }}
        </p>
        <template v-else>
          <div
            v-for="row in associationRows"
            :key="row.linkId"
            class="po-association"
            :data-testid="`association-${row.linkId}`"
          >
            <span class="po-association__name">
              {{ row.project?.name ?? `#${row.projectId.slice(0, 8)}` }}
            </span>
            <Badge v-if="row.project" variant="neutral">
              {{ row.project.status }}
            </Badge>
            <Button
              v-if="row.project"
              variant="secondary"
              size="sm"
              :data-test="`association-open-${row.linkId}`"
              @click="openProject(row.projectId)"
            >
              {{ $t("team.products.openProject") }}
            </Button>
            <span v-else class="po-association__restricted">
              {{ $t("team.products.restricted") }}
            </span>
          </div>
        </template>
      </section>

      <!-- 成员 / 生命周期入口（O1 底部） -->
      <div class="po-actions">
        <Button
          variant="secondary"
          size="sm"
          data-test="goto-members"
          @click="$router.push(`/settings/team/products/${productId}/members`)"
        >
          {{ $t("team.products.manageMembers") }}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          data-test="goto-lifecycle"
          @click="
            $router.push(`/settings/team/products/${productId}/lifecycle`)
          "
        >
          {{ $t("team.products.viewLifecycle") }}
        </Button>
      </div>

      <!-- Activity（O1 页脚锚点；PA-05-08 Product Feed 视图，只读展示） -->
      <section class="po-card" data-testid="product-activity">
        <h2 class="po-card__title">{{ $t("team.products.activityTitle") }}</h2>
        <p v-if="activity.length === 0" class="po-card__empty">
          {{ $t("team.products.activityEmpty") }}
        </p>
        <p
          v-for="row in activity"
          :key="row.id"
          class="po-activity"
          :class="{ 'is-unread': row.read_at === null }"
          :data-testid="`product-activity-${row.id}`"
        >
          <span
            v-if="row.read_at === null"
            class="po-activity__dot"
            aria-hidden="true"
          />
          <span class="po-activity__summary">{{ row.summary }}</span>
          <span class="po-activity__time">
            {{
              $t("team.products.activityMeta", {
                kind: row.activity_type,
                time: relative(row.created_at)
              })
            }}
          </span>
        </p>
      </section>
    </template>
  </PageFrame>
</template>

<style scoped>
.po-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.po-badges {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.po-summary {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-5);
}

.po-summary__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.po-summary__desc {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
  white-space: pre-line;
}

.po-summary__meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-5);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.po-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.po-card__title {
  margin: 0 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.po-card__empty {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.po-association {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.po-association:last-child {
  border-bottom: none;
}

.po-association__name {
  flex: 1;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.po-association__restricted {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.po-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

/* Activity（PA-05-08 Timeline 呈现；未读点 = 个人已读状态提示） */
.po-activity {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.po-activity:last-child {
  border-bottom: none;
}

.po-activity__dot {
  flex: none;
  width: 0.5rem;
  height: 0.5rem;
  align-self: center;
  border-radius: var(--radius-full);
  background: var(--ws-accent);
}

.po-activity__summary {
  flex: 1;
  color: var(--text-primary);
}

.po-activity__time {
  flex: none;
  color: var(--text-tertiary);
}
</style>
