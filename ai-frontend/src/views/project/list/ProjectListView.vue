<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { FolderOpen, Plus, SearchX, TriangleAlert } from "lucide-vue-next";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";
import { PageFrame } from "@/components/shell";

import { loadProjectEntries } from "./fixtures";
import {
  isRecentlyActive,
  parseTimestamp,
  type ProjectActionKind,
  type ProjectEntry,
  type ProjectsLoader
} from "./project-entry";
import OverviewMetrics from "./components/OverviewMetrics.vue";
import ProjectToolbar from "./components/ProjectToolbar.vue";
import ProjectRows from "./components/ProjectRows.vue";
import ProjectGrid from "./components/ProjectGrid.vue";

/**
 * Projects List 页根（mock 先行）。
 *
 * 结构 = 页头（+ New Project）→ 概览指标条 → 工具栏（搜索/过滤/排序/视图）
 * → 列表或网格 → 两级空态（无项目 / 无结果）→ 加载骨架与错误重试。
 *
 * 红线（Projects List §13 + S01-02/S01-03 裁决）：
 *   - 过滤只用域支持的生命周期（all / active / archived），placeholder 归入进行中；
 *   - 仓库只做摘要展示（多仓库 / 无主仓库 / 零仓库均可），不做仓库管理；
 *   - 删除为不可逆操作，必须经 ConfirmationDialog 显式确认；归档可逆、直接执行。
 */
interface Props {
  /** 数据加载器（测试注入；缺省 = fixtures 演示数据） */
  loader?: ProjectsLoader;
}

const props = defineProps<Props>();

const { t } = useI18n();
const router = useRouter();

const phase = ref<"loading" | "ready" | "error">("loading");
const projects = ref<ProjectEntry[]>([]);

const search = ref("");
const lifecycle = ref("all");
const sortKey = ref("updated_desc");
const viewMode = ref<"list" | "grid">("list");
const attentionOnly = ref(false);

const pendingDeleteId = ref<string | null>(null);

async function load() {
  phase.value = "loading";
  try {
    projects.value = await (props.loader ?? loadProjectEntries)();
    phase.value = "ready";
  } catch {
    phase.value = "error";
  }
}

onMounted(load);

const counts = computed(() => ({
  total: projects.value.length,
  recent: projects.value.filter(entry => isRecentlyActive(entry, Date.now()))
    .length,
  attention: projects.value.filter(entry => entry.repositories.has_unreachable)
    .length,
  archived: projects.value.filter(entry => entry.status === "archived").length
}));

/** 指标条高亮键：all / active / attention / archived */
const metricActive = computed(() =>
  attentionOnly.value ? "attention" : lifecycle.value
);

const visibleProjects = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  const filtered = projects.value.filter(entry => {
    if (lifecycle.value === "active" && entry.status === "archived")
      return false;
    if (lifecycle.value === "archived" && entry.status !== "archived")
      return false;
    if (attentionOnly.value && !entry.repositories.has_unreachable)
      return false;
    if (keyword) {
      const haystack =
        `${entry.code} ${entry.name} ${entry.description}`.toLowerCase();
      if (!haystack.includes(keyword)) return false;
    }
    return true;
  });

  return [...filtered].sort((a, b) => {
    if (sortKey.value === "created_desc") {
      return parseTimestamp(b.created_at) - parseTimestamp(a.created_at);
    }
    if (sortKey.value === "name_asc") {
      return a.name.localeCompare(b.name);
    }
    return parseTimestamp(b.updated_at) - parseTimestamp(a.updated_at);
  });
});

function onMetricSelect(key: string) {
  if (key === "attention") {
    attentionOnly.value = !attentionOnly.value;
    return;
  }
  attentionOnly.value = false;
  lifecycle.value = key;
}

function resetFilters() {
  search.value = "";
  lifecycle.value = "all";
  attentionOnly.value = false;
}

function setEntryStatus(id: string, status: ProjectEntry["status"]) {
  const entry = projects.value.find(item => item.id === id);
  if (entry) entry.status = status;
}

function onAction(kind: ProjectActionKind, id: string) {
  if (kind === "open") {
    router.push("/project/command-center");
    return;
  }
  if (kind === "archive") {
    setEntryStatus(id, "archived");
    return;
  }
  if (kind === "restore") {
    setEntryStatus(id, "active");
    return;
  }
  pendingDeleteId.value = id;
}

const pendingDelete = computed(
  () => projects.value.find(entry => entry.id === pendingDeleteId.value) ?? null
);

function onConfirmDelete() {
  const id = pendingDeleteId.value;
  if (id) {
    projects.value = projects.value.filter(entry => entry.id !== id);
  }
  pendingDeleteId.value = null;
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('pm.projects.pageTitle') }]"
    :title="t('pm.projects.pageTitle')"
    :description="t('pm.projects.subtitle')"
  >
    <template #actions>
      <Button
        variant="primary"
        size="sm"
        @click="router.push('/project/create')"
      >
        <Plus aria-hidden="true" />
        {{ t("pm.projects.new") }}
      </Button>
    </template>

    <OverviewMetrics
      :counts="counts"
      :active="metricActive"
      @select="onMetricSelect"
    />

    <ProjectToolbar
      :search="search"
      :lifecycle="lifecycle"
      :sort-key="sortKey"
      :view-mode="viewMode"
      :attention-only="attentionOnly"
      @update:search="search = $event"
      @update:lifecycle="lifecycle = $event"
      @update:sort-key="sortKey = $event"
      @update:view-mode="viewMode = $event"
      @remove-attention="attentionOnly = false"
    />

    <div
      v-if="phase === 'loading'"
      class="proj-skeleton"
      role="status"
      :aria-label="t('pm.projects.loadingAria')"
    >
      <div v-for="index in 5" :key="index" class="proj-skeleton__row">
        <Skeleton width="var(--space-6)" height="var(--space-6)" />
        <div class="proj-skeleton__lines">
          <Skeleton
            tone="strong"
            width="30%"
            height="var(--line-height-body)"
          />
          <Skeleton width="60%" height="var(--line-height-caption)" />
        </div>
        <Skeleton width="18%" height="var(--line-height-caption)" />
      </div>
    </div>

    <EmptyState
      v-else-if="phase === 'error'"
      :title="t('pm.projects.errorTitle')"
      :description="t('pm.projects.errorDesc')"
      :icon="TriangleAlert"
      :action-label="t('pm.projects.errorCta')"
      @action="load"
    />

    <template v-else>
      <EmptyState
        v-if="projects.length === 0"
        :title="t('pm.projects.emptyTitle')"
        :description="t('pm.projects.emptyDesc')"
        :icon="FolderOpen"
        :action-label="t('pm.projects.emptyCta')"
        @action="router.push('/project/create')"
      />
      <template v-else-if="visibleProjects.length === 0">
        <EmptyState
          :title="t('pm.projects.noResultsTitle')"
          :description="t('pm.projects.noResultsDesc')"
          :icon="SearchX"
          :action-label="t('pm.projects.noResultsCta')"
          @action="resetFilters"
        />
      </template>
      <template v-else>
        <ProjectRows
          v-if="viewMode === 'list'"
          :entries="visibleProjects"
          @open="id => onAction('open', id)"
          @action="onAction"
        />
        <ProjectGrid
          v-else
          :entries="visibleProjects"
          @open="id => onAction('open', id)"
          @action="onAction"
        />
      </template>
    </template>

    <ConfirmationDialog
      :open="pendingDeleteId !== null"
      :title="t('pm.projects.deleteTitle')"
      :description="
        pendingDelete
          ? t('pm.projects.deleteDesc', { name: pendingDelete.name })
          : ''
      "
      :confirm-label="t('pm.projects.menuDelete')"
      :cancel-label="t('common.cancel')"
      variant="danger"
      @update:open="value => !value && (pendingDeleteId = null)"
      @confirm="onConfirmDelete"
      @cancel="pendingDeleteId = null"
    />
  </PageFrame>
</template>

<style scoped>
.proj-skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
}

.proj-skeleton__row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.proj-skeleton__lines {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  flex: 1 1 0;
}
</style>
