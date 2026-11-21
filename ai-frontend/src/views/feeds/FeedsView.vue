<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Bot,
  ChevronDown,
  ChevronUp,
  FileText,
  User,
  Cog
} from "lucide-vue-next";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { EmptyState } from "@/components/ui/empty-state";
import {
  fetchMyFeed,
  fetchProductFeed,
  fetchProjectFeed,
  markAllFeedRead,
  markFeedRead,
  type FeedCounts,
  type FeedItem,
  type FeedView
} from "@/api/feeds";
import { listProjects } from "@/api/pm";
import { listProducts } from "@/api/products";
import { useTeamStore } from "@/stores/team";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";

/**
 * 统一信息流页（PA-05 UI 01/02/03：My / Product / Project activity feed）。
 *
 * 页内三视角 tabs（同一 Activity 数据的不同视图，不另起数据源）：
 *   - My feed：Membership 路由去重集 + All/Unread/Needs attention 视图条
 *     + Mark all as read（07 已读管理并入页头，PA-05-09 批量标记）；
 *   - Product feed / Project feed：选定产品/项目后呈现维度全量活动。
 * 筛选栏：Activity type / Actor（画布四筛选中时间范围 V1 缺省最新优先）。
 * 活动卡行内展开 detail（画布「Expand ↓」），关联事项引用跳领域页。
 */
const { t } = useI18n();
const router = useRouter();
const teamStore = useTeamStore();
const toast = useToast();

type Scope = "my" | "product" | "project";
const scope = ref<Scope>("my");
const view = ref<FeedView>("all");
const items = ref<FeedItem[]>([]);
const counts = ref<FeedCounts | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const expandedId = ref<string | null>(null);
const offset = ref(0);
const hasMore = ref(false);
const PAGE = 20;

const products = ref<Array<{ id: string; name: string }>>([]);
const projects = ref<Array<{ id: string; name: string }>>([]);
const selectedProductId = ref("");
const selectedProjectId = ref("");
const actorFilter = ref("");
const activityTypeFilter = ref("");

const viewTabs = computed(() => [
  { key: "all", label: t("pm.feeds.viewAll") },
  {
    key: "unread",
    label: `${t("pm.feeds.viewUnread")} · ${counts.value?.unread ?? 0}`
  },
  {
    key: "attention",
    label: `${t("pm.feeds.viewAttention")} · ${counts.value?.attention ?? 0}`
  }
]);

// reka-ui SelectItem 禁止空串 value：用 "all" 哨兵，请求层还原为 undefined。
const ACTOR_ALL = "all";
const TYPE_ALL = "all";
const actorOptions = [
  { value: ACTOR_ALL, label: t("pm.feeds.actorAll") },
  { value: "human", label: t("pm.feeds.actorHuman") },
  { value: "agent", label: t("pm.feeds.actorAgent") }
];

const productOptions = computed(() =>
  products.value.map(p => ({ value: p.id, label: p.name }))
);
const projectOptions = computed(() =>
  projects.value.map(p => ({ value: p.id, label: p.name }))
);

const pageTitle = computed(() =>
  scope.value === "my"
    ? t("pm.feeds.titleMy")
    : scope.value === "product"
      ? t("pm.feeds.titleProduct")
      : t("pm.feeds.titleProject")
);

const scopeMissing = computed(
  () =>
    (scope.value === "product" && !selectedProductId.value) ||
    (scope.value === "project" && !selectedProjectId.value)
);

function actorIcon(kind: string) {
  return kind === "agent" ? Bot : kind === "system" ? Cog : User;
}

function toggleExpand(row: FeedItem): void {
  expandedId.value = expandedId.value === row.id ? null : row.id;
}

/** 关联事项跳转：requirement → 需求详情；adr → 设计域；其余 → 项目总控台 */
function openTarget(row: FeedItem): void {
  if (row.target_type === "requirement") {
    void router.push(`/project/requirements/${row.target_id}`);
  } else if (row.target_type === "adr") {
    void router.push("/project/design");
  } else if (row.project_id) {
    void router.push(`/project/command-center?project_id=${row.project_id}`);
  }
}

async function load(): Promise<void> {
  if (scopeMissing.value) {
    items.value = [];
    counts.value = null;
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    if (scope.value === "my") {
      const res = await fetchMyFeed({
        limit: PAGE,
        offset: offset.value,
        view: view.value,
        actor_type:
          actorFilter.value && actorFilter.value !== ACTOR_ALL
            ? actorFilter.value
            : undefined,
        activity_type:
          activityTypeFilter.value && activityTypeFilter.value !== TYPE_ALL
            ? activityTypeFilter.value
            : undefined,
        project_id: selectedProjectId.value || undefined
      });
      items.value = res.items;
      counts.value = res.counts ?? null;
    } else if (scope.value === "product" && selectedProductId.value) {
      items.value = await fetchProductFeed(
        selectedProductId.value,
        PAGE,
        offset.value
      );
      counts.value = null;
    } else if (scope.value === "project" && selectedProjectId.value) {
      items.value = await fetchProjectFeed(
        selectedProjectId.value,
        PAGE,
        offset.value
      );
      counts.value = null;
    }
    hasMore.value = items.value.length === PAGE;
  } catch (err) {
    error.value = translateError(err);
  } finally {
    loading.value = false;
  }
}

function switchScope(next: Scope): void {
  scope.value = next;
  offset.value = 0;
  expandedId.value = null;
  void load();
}

function switchView(next: FeedView): void {
  view.value = next;
  offset.value = 0;
  void load();
}

function loadEarlier(): void {
  offset.value += PAGE;
  void load();
}

async function markAll(): Promise<void> {
  try {
    await markAllFeedRead();
    toast.toast({ title: t("pm.feeds.markAllDone") });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  }
}

async function markRead(row: FeedItem, read: boolean): Promise<void> {
  try {
    await markFeedRead([row.item_id], read);
    row.read_at = read ? new Date().toISOString() : null;
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  }
}

onMounted(async () => {
  await teamStore.load();
  try {
    const teamId = teamStore.context?.team?.id;
    if (teamId) {
      const [productRows, projectRows] = await Promise.all([
        listProducts(teamId),
        listProjects({ page: 1, size: 200 })
      ]);
      products.value = productRows.map(p => ({ id: p.id, name: p.name }));
      projects.value = (projectRows.items ?? []).map(p => ({
        id: p.id,
        name: p.name
      }));
    }
  } catch {
    // 维度下拉加载失败不阻塞 My feed 主视图。
  }
  await load();
});

watch(
  [selectedProductId, selectedProjectId, actorFilter, activityTypeFilter],
  () => {
    offset.value = 0;
    void load();
  }
);
</script>

<template>
  <PageFrame :title="pageTitle" :description="$t('pm.feeds.subtitle')">
    <!-- 视角 tabs（01/02/03 同构） -->
    <nav class="fd-tabs" :aria-label="$t('pm.feeds.tabsLabel')">
      <button
        v-for="tab in ['my', 'product', 'project'] as const"
        :key="tab"
        type="button"
        class="fd-tabs__item"
        :class="{ 'is-active': scope === tab }"
        :aria-current="scope === tab ? 'page' : undefined"
        @click="switchScope(tab)"
      >
        {{ $t(`pm.feeds.tab.${tab}`) }}
      </button>
    </nav>

    <!-- 页头：未读徽标 + Mark all as read（07 已读管理并入） -->
    <div v-if="scope === 'my'" class="fd-head">
      <Badge variant="neutral">
        {{ $t("pm.feeds.unreadBadge", { count: counts?.unread ?? 0 }) }}
      </Badge>
      <Button
        variant="secondary"
        size="sm"
        data-test="feeds-mark-all"
        @click="markAll"
      >
        {{ $t("pm.feeds.markAll") }}
      </Button>
    </div>

    <!-- 视图条（My feed） -->
    <div v-if="scope === 'my'" class="fd-views">
      <button
        v-for="tab in viewTabs"
        :key="tab.key"
        type="button"
        class="fd-views__item"
        :class="{ 'is-active': view === tab.key }"
        @click="switchView(tab.key as FeedView)"
      >
        {{ tab.label }}
      </button>
    </div>

    <!-- 筛选栏 -->
    <div class="fd-filters">
      <Select
        v-if="scope === 'product'"
        v-model="selectedProductId"
        :options="productOptions"
        :placeholder="$t('pm.feeds.pickProduct')"
        :aria-label="$t('pm.feeds.pickProduct')"
        class="fd-filter"
      />
      <Select
        v-if="scope === 'project'"
        v-model="selectedProjectId"
        :options="projectOptions"
        :placeholder="$t('pm.feeds.pickProject')"
        :aria-label="$t('pm.feeds.pickProject')"
        class="fd-filter"
      />
      <Select
        v-model="activityTypeFilter"
        :options="[
          { value: 'all', label: $t('pm.feeds.typeAll') },
          { value: 'created', label: $t('pm.feeds.typeCreated') },
          { value: 'status_changed', label: $t('pm.feeds.typeStatusChanged') },
          {
            value: 'relation_changed',
            label: $t('pm.feeds.typeRelationChanged')
          },
          { value: 'acceptance_recorded', label: $t('pm.feeds.typeAcceptance') }
        ]"
        :aria-label="$t('pm.feeds.filterType')"
        class="fd-filter"
      />
      <Select
        v-model="actorFilter"
        :options="actorOptions"
        :aria-label="$t('pm.feeds.filterActor')"
        class="fd-filter"
      />
    </div>

    <p v-if="error" class="fd-error">{{ error }}</p>

    <!-- 活动列表 -->
    <EmptyState
      v-if="!loading && items.length === 0"
      :title="$t('pm.feeds.emptyTitle')"
      :description="$t('pm.feeds.emptyHint')"
    />
    <template v-else>
      <article
        v-for="row in items"
        :key="row.id"
        class="fd-item"
        :class="{ 'is-unread': row.read_at === null }"
        :data-testid="`feed-item-${row.id}`"
      >
        <div class="fd-item__head">
          <component
            :is="actorIcon(row.actor_type)"
            :size="14"
            aria-hidden="true"
          />
          <span class="fd-item__actor">{{
            row.actor_type === "agent"
              ? $t("pm.feeds.actorAgent")
              : $t("pm.feeds.actorHuman")
          }}</span>
          <span class="fd-item__time">{{
            new Date(row.created_at).toLocaleString()
          }}</span>
          <Badge variant="neutral">{{ row.activity_type }}</Badge>
          <Badge v-if="row.read_at === null" variant="accent">
            {{ $t("pm.feeds.unread") }}
          </Badge>
          <button
            v-else
            type="button"
            class="fd-item__mark"
            @click="markRead(row, false)"
          >
            {{ $t("pm.feeds.markUnread") }}
          </button>
        </div>
        <h3 class="fd-item__title">{{ row.item_title }}</h3>
        <p class="fd-item__summary">{{ row.summary }}</p>
        <div class="fd-item__foot">
          <button type="button" class="fd-item__link" @click="openTarget(row)">
            <FileText :size="13" aria-hidden="true" />
            {{ row.target_type }} · {{ row.item_title }}
          </button>
          <button
            v-if="row.detail"
            type="button"
            class="fd-item__expand"
            @click="toggleExpand(row)"
          >
            {{
              expandedId === row.id
                ? $t("pm.feeds.collapse")
                : $t("pm.feeds.expand")
            }}
            <component
              :is="expandedId === row.id ? ChevronUp : ChevronDown"
              :size="13"
              aria-hidden="true"
            />
          </button>
        </div>
        <pre v-if="expandedId && row.detail" class="fd-item__detail">{{
          JSON.stringify(row.detail, null, 2)
        }}</pre>
      </article>

      <div v-if="hasMore" class="fd-more">
        <span class="fd-more__note">{{
          $t("pm.feeds.showing", { count: items.length })
        }}</span>
        <Button
          variant="secondary"
          size="sm"
          data-test="feeds-load-earlier"
          @click="loadEarlier"
        >
          {{ $t("pm.feeds.loadEarlier") }}
        </Button>
      </div>
    </template>
  </PageFrame>
</template>

<style scoped>
.fd-tabs {
  display: flex;
  align-items: stretch;
  border-bottom: 1px solid var(--ws-border);
}

.fd-tabs__item {
  padding: 0 var(--space-3);
  height: 2.25rem;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
  cursor: pointer;
}

.fd-tabs__item.is-active {
  border-bottom-color: var(--ws-accent);
  color: var(--text-primary);
}

.fd-tabs__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.fd-head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.fd-views {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: 1px solid var(--ws-border);
}

.fd-views__item {
  padding: 0 var(--space-3);
  height: 2.25rem;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
  cursor: pointer;
}

.fd-views__item.is-active {
  border-bottom-color: var(--ws-accent);
  color: var(--text-primary);
}

.fd-filters {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.fd-filter {
  min-width: 12rem;
  flex: 0 1 14rem;
}

.fd-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.fd-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-3) var(--space-4);
}

.fd-item.is-unread {
  border-color: var(--ws-accent);
}

.fd-item__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.fd-item__actor {
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.fd-item__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.fd-item__summary {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-secondary);
}

.fd-item__foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.fd-item__link {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  border: none;
  background: transparent;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--ws-accent);
  cursor: pointer;
}

.fd-item__expand {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  border: none;
  background: transparent;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-accent);
  cursor: pointer;
}

.fd-item__detail {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  font-family: var(--font-mono, monospace);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  white-space: pre-wrap;
}

.fd-item__mark {
  border: none;
  background: transparent;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
  cursor: pointer;
}

.fd-more {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.fd-more__note {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
