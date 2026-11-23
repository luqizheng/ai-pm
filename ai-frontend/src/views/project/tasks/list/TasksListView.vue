<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import PageFrame from "@/components/shell/PageFrame.vue";
import Button from "@/components/ui/button/Button.vue";
import SearchInput from "@/components/ui/search-input/SearchInput.vue";
import Select from "@/components/ui/select/Select.vue";
import Table from "@/components/ui/table/Table.vue";
import TableBody from "@/components/ui/table/TableBody.vue";
import TableCell from "@/components/ui/table/TableCell.vue";
import TableHead from "@/components/ui/table/TableHead.vue";
import TableHeader from "@/components/ui/table/TableHeader.vue";
import TableRow from "@/components/ui/table/TableRow.vue";
import { listTasks } from "@/api/tasks";
import { listProjectProducts } from "@/api/products";
import type { Task } from "@/api/types";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import TasksFrame from "../TasksFrame.vue";
import {
  aggregateStatusLabel,
  TASK_STATUSES,
  priorityTone
} from "../tasks-contract";
import {
  DEPENDENCY_FILTERS,
  filterRows,
  productOptions,
  toListRow,
  type RelatedProduct,
  type TaskListRow
} from "./task-list-adapter";
import SchedulingConditionChip from "./SchedulingConditionChip.vue";
import TaskExecutionPanel from "./TaskExecutionPanel.vue";

/**
 * PA-07 任务列表页（Figma 179:3603 A「Project Task List」）。
 *
 * 任务生命周期、执行与调度条件一屏：五列表格 + 三组筛选 + 搜索 +
 * List/Relationships 视图切换 + Execution context 右栏。
 * 排序 = 后端（priority urgent→low + created_at DESC）；筛选 = 客户端
 * （TasksBacklogView 同口径，size=200 全量拉取）；调度条件是派生展示
 * （tasks-contract schedulingCondition），绝不替代任务生命周期状态。
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

const rows = ref<TaskListRow[]>([]);
const loading = ref(false);
const loadError = ref<string | null>(null);
const updatedAt = ref<string | null>(null);

const FILTER_ALL = "__all__";
const productFilter = ref(FILTER_ALL);
const statusFilter = ref(FILTER_ALL);
const dependencyFilter = ref(FILTER_ALL);
const searchText = ref("");

const relatedProducts = ref<RelatedProduct[]>([]);
const panelLoading = ref(false);

async function load() {
  loading.value = true;
  loadError.value = null;
  try {
    const res = await listTasks({
      project_id: projectId.value || undefined,
      size: 200
    });
    rows.value = (res.items ?? []).map(toListRow);
    updatedAt.value = rows.value.reduce<string | null>(
      (latest, row) =>
        !latest || row.updatedAt > latest ? row.updatedAt : latest,
      null
    );
  } catch (error) {
    loadError.value = describeError(error);
    rows.value = [];
  } finally {
    loading.value = false;
  }
}

async function loadPanel() {
  if (!projectId.value) {
    relatedProducts.value = [];
    return;
  }
  panelLoading.value = true;
  try {
    const items = await listProjectProducts(projectId.value);
    relatedProducts.value = items.map(item => ({
      id: item.product.id,
      name: item.product.name,
      status: item.product.status,
      taskCount: item.task_count
    }));
  } catch {
    // 右栏产品列表失败不阻塞主列表：诚实空态。
    relatedProducts.value = [];
  } finally {
    panelLoading.value = false;
  }
}

onMounted(() => {
  void load();
  void loadPanel();
});

watch(projectId, () => {
  void load();
  void loadPanel();
});

const productChoices = computed(() => [
  { value: FILTER_ALL, label: t("pa07.list.filterProductAll") },
  ...productOptions(rows.value)
]);

const statusChoices = computed(() => [
  { value: FILTER_ALL, label: t("pa07.list.filterStatusAll") },
  ...TASK_STATUSES.map(meta => ({
    value: meta.value,
    label: t(`s04.tasks.status.${meta.value}`)
  }))
]);

const dependencyChoices = computed(() => [
  { value: FILTER_ALL, label: t("pa07.list.filterDependencyAll") },
  ...DEPENDENCY_FILTERS.map(key => ({
    value: key,
    label: t(`pa07.condition.${key}`)
  }))
]);

const filteredRows = computed(() =>
  filterRows(rows.value, {
    product: productFilter.value,
    status: statusFilter.value,
    dependency: dependencyFilter.value,
    search: searchText.value
  })
);

const hasAnyFilter =
  productFilter.value !== FILTER_ALL ||
  statusFilter.value !== FILTER_ALL ||
  dependencyFilter.value !== FILTER_ALL ||
  searchText.value.length > 0;

function clearFilters() {
  productFilter.value = FILTER_ALL;
  statusFilter.value = FILTER_ALL;
  dependencyFilter.value = FILTER_ALL;
  searchText.value = "";
}

function openTask(row: TaskListRow) {
  void router.push(`/project/tasks/${row.id}`);
}

const countLabel = computed(() =>
  t("pa07.list.count", { count: filteredRows.value.length })
);

function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
</script>

<template>
  <div class="tasks-list-page">
    <div class="tasks-list-main">
      <PageFrame
        :title="t('pa07.list.title')"
        :description="t('pa07.list.subtitle')"
      >
        <TasksFrame />

        <div class="tl-views" data-testid="list-views">
          <RouterLink
            to="/project/tasks/list"
            class="tl-views__tab is-active"
            aria-current="page"
            :data-testid="'list-view-list'"
          >
            {{ t("pa07.list.viewList") }}
          </RouterLink>
          <RouterLink
            to="/project/tasks/relationships"
            class="tl-views__tab"
            :data-testid="'list-view-relationships'"
          >
            {{ t("pa07.list.viewRelationships") }}
          </RouterLink>
        </div>

        <div class="tl-toolbar">
          <Select
            v-model="productFilter"
            :options="productChoices"
            :aria-label="t('pa07.list.filterProduct')"
            data-testid="filter-product"
          />
          <Select
            v-model="statusFilter"
            :options="statusChoices"
            :aria-label="t('pa07.list.filterStatus')"
            data-testid="filter-status"
          />
          <Select
            v-model="dependencyFilter"
            :options="dependencyChoices"
            :aria-label="t('pa07.list.filterDependency')"
            data-testid="filter-dependency"
          />
          <Button
            v-if="hasAnyFilter"
            variant="secondary"
            size="md"
            data-test="list-clear-filters"
            @click="clearFilters"
          >
            {{ t("pa07.list.clearFilters") }}
          </Button>
          <SearchInput
            v-model="searchText"
            class="tl-toolbar__search"
            :placeholder="t('pa07.list.searchPlaceholder')"
            :aria-label="t('pa07.list.searchPlaceholder')"
            data-test="list-search"
          />
        </div>

        <div class="tl-meta" data-testid="list-count">
          <span>{{ countLabel }}</span>
          <span class="tl-meta__sort">{{
            t("pa07.list.sortedByPriority")
          }}</span>
        </div>

        <p v-if="loadError" class="tl-error">{{ loadError }}</p>
        <p v-else-if="loading" class="tl-note">{{ t("common.loading") }}</p>

        <Table v-else class="tl-table" data-test="task-execution-table">
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("pa07.list.colTask") }}</TableHead>
              <TableHead>{{ t("pa07.list.colProducts") }}</TableHead>
              <TableHead>{{ t("pa07.list.colStatus") }}</TableHead>
              <TableHead>{{ t("pa07.list.colExecution") }}</TableHead>
              <TableHead>{{ t("pa07.list.colCondition") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="row in filteredRows"
              :key="row.id"
              data-test="task-list-row"
              @click="openTask(row)"
            >
              <TableCell class="tl-cell-task">
                <span
                  class="tl-task-code"
                  :data-testid="`task-code-${row.code}`"
                  >{{ row.code }}</span
                >
                <span class="tl-task-title">{{ row.title }}</span>
                <span
                  class="tl-task-meta"
                  :class="`tl-task-meta--${priorityTone(row.priority)}`"
                >
                  {{ row.grade
                  }}<template v-if="row.due"> · {{ row.due }}</template>
                </span>
              </TableCell>
              <TableCell class="tl-cell-products">
                <template v-if="row.productNames">{{
                  row.productNames
                }}</template>
                <template v-else>—</template>
              </TableCell>
              <TableCell>
                <span class="tl-status" :class="`tl-status--${row.statusTone}`">
                  {{ row.statusLabel }}
                </span>
              </TableCell>
              <TableCell class="tl-cell-execution">
                <span class="tl-exec">{{ row.execution.label }}</span>
                <span v-if="row.execution.detail" class="tl-exec__detail">{{
                  row.execution.detail
                }}</span>
              </TableCell>
              <TableCell class="tl-cell-condition">
                <SchedulingConditionChip :condition="row.condition" />
              </TableCell>
            </TableRow>
            <TableRow v-if="filteredRows.length === 0">
              <TableCell colspan="5" class="tl-empty" data-test="list-empty">
                {{ t("pa07.list.empty") }}
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>

        <p class="tl-footnote">
          {{ t("pa07.list.footnote") }}
        </p>
      </PageFrame>
    </div>

    <TaskExecutionPanel
      :products="relatedProducts"
      :updated-at="updatedAt"
      :loading="panelLoading"
      class="tasks-list-panel"
    />
  </div>
</template>

<style scoped>
/* 页面级双栏：主列表 + Execution context 右栏（设计稿 304 面板口径） */
.tasks-list-page {
  display: flex;
  align-items: stretch;
  min-height: 0;
}

.tasks-list-main {
  flex: 1;
  min-width: 0;
}

.tasks-list-panel {
  width: 304px;
  flex-shrink: 0;
}

/* List / Relationships 二级切换：PlanningViewTabs 同口径 */
.tl-views {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.tl-views__tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-text-secondary);
  text-decoration: none;
  white-space: nowrap;
}

.tl-views__tab.is-active {
  color: var(--ws-text-primary);
}

.tl-views__tab.is-active::after {
  position: absolute;
  right: 0;
  bottom: calc(-1 * var(--space-0));
  left: 0;
  height: var(--size-indicator);
  background: var(--ws-accent);
  content: "";
}

.tl-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

.tl-toolbar__search {
  max-width: 400px;
  margin-left: auto;
}

.tl-meta {
  display: flex;
  justify-content: space-between;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.tl-meta__sort {
  color: var(--ws-text-disabled);
}

.tl-error {
  margin: 0;
  color: var(--ws-color-danger-text);
}

.tl-note {
  margin: 0;
  color: var(--ws-text-secondary);
}

.tl-cell-task {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.tl-task-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.tl-task-title {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 500;
  color: var(--ws-text-primary);
}

.tl-task-meta {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.tl-task-meta--danger {
  color: var(--ws-color-danger-text);
}

.tl-task-meta--warning {
  color: var(--ws-color-warning-text);
}

.tl-task-meta--muted {
  color: var(--ws-text-disabled);
}

.tl-cell-products {
  white-space: pre-line;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-primary);
}

.tl-status {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.tl-status--success {
  color: var(--ws-color-success-text);
}

.tl-status--danger {
  color: var(--ws-color-danger-text);
}

.tl-status--info {
  color: var(--ws-color-info-text);
}

.tl-status--warning {
  color: var(--ws-color-warning-text);
}

.tl-status--muted {
  color: var(--ws-text-disabled);
}

.tl-status--ready {
  color: var(--ws-color-status-ready);
}

.tl-status--neutral {
  color: var(--ws-text-primary);
}

.tl-cell-execution {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.tl-exec {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-primary);
}

.tl-exec__detail {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.tl-empty {
  text-align: center;
  color: var(--ws-text-secondary);
}

.tl-footnote {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-disabled);
}
</style>
