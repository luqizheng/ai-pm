<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import {
  ChevronLeft,
  ChevronRight,
  Columns3,
  GitBranch,
  List,
  Pencil,
  Plus,
  RefreshCw
} from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import Dialog from "@/components/ui/dialog/Dialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Input from "@/components/ui/input/Input.vue";
import Select from "@/components/ui/select/Select.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import { PageFrame } from "@/components/shell";
import { HttpError } from "@digua/http";
import {
  addTaskDependency,
  createTask,
  listTaskDependencies,
  listTasks,
  markTasksReady,
  removeTaskDependency,
  updateTask,
  type TaskDependencyEdge,
  type TaskDependencyGraph
} from "@/api/tasks";
import type { Task } from "@/api/types";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import TaskStatusChip from "../components/TaskStatusChip.vue";
import {
  KANBAN_LANES,
  TASK_PRIORITIES,
  TASK_STATUSES,
  formatDateTime,
  shortId,
  toTaskRow,
  type TaskRowModel
} from "../tasks-contract";

/**
 * 任务池页根（S04-UI-04，2026-11-10 落地，接真）。
 *
 * 数据 = pm-api REST：
 *   - 列表 /v1/pm/tasks（size 上限 200；筛选为当前页客户端过滤，
 *     服务端筛选参数待补，页面不展示伪功能）；
 *   - 新建 / 编辑走 createTask / updateTask（revision 乐观锁，409 显式提示）；
 *   - 批量「置为待执行」必须走 mark-ready 端点（E5：直写不产生 TaskReady
 *     事件，任务不进调度），仅草稿可批量；
 *   - 依赖增删走 /v1/pm/tasks/{id}/dependencies（环路 409 原样呈现）；
 *   - 看板拖拽改状态 = updateTask（后端 13 态状态机校验，非法迁移被拒）。
 * 仓库 / 工作量列无真实读模型，以「—」呈现，不做演示数据填充。
 */
const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const scope = useScopeStore();

const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  if (scope.currentProjectId && scope.currentProjectId !== SCOPE_ALL) {
    return scope.currentProjectId;
  }
  return "";
});

const PAGE_SIZE = 50;

const rows = ref<TaskRowModel[]>([]);

/** TASK-01 §3 执行类型派生：有 run 记录=AI 执行；无 run=人工/未指派。 */
const EXEC_RUN_ACTIVE = new Set([
  "claimed",
  "running",
  "submitted",
  "reviewing",
  "awaiting_decision"
]);

function executionKind(task: TaskRowModel): "ai" | "human" | "unassigned" {
  if (task.runStatus) return "ai";
  if (!task.assigneeId) return "unassigned";
  return "human";
}
const total = ref(0);
const page = ref(1);
const loading = ref(true);
const loadError = ref(false);

/** 「全部」哨兵：reka-ui Select 禁止空串 value（同 scope.ts 的 SCOPE_ALL 先例） */
const FILTER_ALL = "__all__";

const statusFilter = ref(FILTER_ALL);
const priorityFilter = ref(FILTER_ALL);
const unassignedOnly = ref(false);
const searchText = ref("");
const viewMode = ref<"table" | "kanban">("table");

const statusOptions = computed(() => [
  { value: FILTER_ALL, label: t("s04.tasks.common.all") },
  ...TASK_STATUSES.map(meta => ({
    value: meta.value,
    label: t(`s04.tasks.status.${meta.value}`)
  }))
]);

const priorityOptions = computed(() => [
  { value: FILTER_ALL, label: t("s04.tasks.common.all") },
  ...TASK_PRIORITIES.map(value => ({
    value,
    label: t(`s04.tasks.priority.${value}`)
  }))
]);

async function load() {
  loading.value = true;
  loadError.value = false;
  try {
    const result = await listTasks({
      project_id: projectId.value || undefined,
      page: page.value,
      size: PAGE_SIZE
    });
    rows.value = (result.items ?? []).map(toTaskRow);
    total.value = result.total ?? rows.value.length;
  } catch {
    loadError.value = true;
  } finally {
    loading.value = false;
  }
}

onMounted(applyRouteQuery);
watch(projectId, () => {
  page.value = 1;
  void load();
});
watch(() => route.query, applyRouteQuery);

/** 总览/入口的预过滤（?status= / ?priority= / ?unassigned=1 / ?create=1） */
function applyRouteQuery() {
  const query = route.query;
  statusFilter.value =
    typeof query.status === "string" && query.status
      ? query.status
      : FILTER_ALL;
  priorityFilter.value =
    typeof query.priority === "string" && query.priority
      ? query.priority
      : FILTER_ALL;
  unassignedOnly.value = query.unassigned === "1";
  void load();
  if (query.create === "1") {
    openCreate();
    void router.replace({ query: { ...query, create: undefined } });
  }
}

const filteredRows = computed(() =>
  rows.value.filter(task => {
    if (statusFilter.value !== FILTER_ALL && task.status !== statusFilter.value)
      return false;
    if (
      priorityFilter.value !== FILTER_ALL &&
      task.priority !== priorityFilter.value
    )
      return false;
    if (unassignedOnly.value && task.assigneeId) return false;
    const keyword = searchText.value.trim().toLowerCase();
    if (keyword) {
      const haystack = `${shortId(task.id)} ${task.title}`.toLowerCase();
      if (!haystack.includes(keyword)) return false;
    }
    return true;
  })
);

function clearFilters() {
  statusFilter.value = FILTER_ALL;
  priorityFilter.value = FILTER_ALL;
  unassignedOnly.value = false;
  searchText.value = "";
}

const hasFilters = computed(
  () =>
    statusFilter.value !== FILTER_ALL ||
    priorityFilter.value !== FILTER_ALL ||
    unassignedOnly.value ||
    searchText.value !== ""
);

// ─── 多选与批量置 ready ────────────────────────────────────────────────

const selected = reactive(new Set<string>());
const markReadyOpen = ref(false);
const acting = ref(false);
const actionError = ref("");
const actionNotice = ref("");

const selectedDrafts = computed(() =>
  rows.value.filter(task => selected.has(task.id) && task.status === "draft")
);

function toggleSelected(id: string, checked: boolean) {
  if (checked) selected.add(id);
  else selected.delete(id);
}

function onRowCheck(task: TaskRowModel, event: Event) {
  toggleSelected(task.id, (event.target as HTMLInputElement).checked);
}

function clearSelection() {
  selected.clear();
}

const markReadyDisabled = computed(
  () => acting.value || selectedDrafts.value.length === 0
);

async function confirmMarkReady() {
  acting.value = true;
  actionError.value = "";
  actionNotice.value = "";
  try {
    const result = await markTasksReady(
      projectId.value,
      selectedDrafts.value.map(task => task.id)
    );
    const outcome = result.result;
    actionNotice.value = t("s04.tasks.backlog.markReadyResult", {
      ready: outcome.ready.length,
      skipped: outcome.skipped.length
    });
    markReadyOpen.value = false;
    clearSelection();
    await load();
  } catch (error) {
    markReadyOpen.value = false;
    actionError.value = describeError(error);
  } finally {
    acting.value = false;
  }
}

function describeError(error: unknown): string {
  if (error instanceof HttpError) {
    if (error.status === 409) return t("s04.tasks.backlog.conflict");
    if (error.message) return error.message;
  }
  return t("s04.tasks.common.actionFailed");
}

// ─── 新建 / 编辑 ───────────────────────────────────────────────────────

const formOpen = ref(false);
const editing = ref<TaskRowModel | null>(null);
const form = reactive({
  title: "",
  description: "",
  status: "draft",
  priority: "normal",
  assignee: ""
});
const formError = ref("");

const statusFormOptions = computed(() =>
  TASK_STATUSES.map(meta => ({
    value: meta.value,
    label: t(`s04.tasks.status.${meta.value}`)
  }))
);

const priorityFormOptions = computed(() =>
  TASK_PRIORITIES.map(value => ({
    value,
    label: t(`s04.tasks.priority.${value}`)
  }))
);

function openCreate() {
  editing.value = null;
  form.title = "";
  form.description = "";
  form.status = "draft";
  form.priority = "normal";
  form.assignee = "";
  formError.value = "";
  formOpen.value = true;
}

function openEdit(task: TaskRowModel) {
  editing.value = task;
  form.title = task.title;
  form.description = task.description ?? "";
  form.status = task.status;
  form.priority = task.priority;
  form.assignee = task.assigneeId ?? "";
  formError.value = "";
  formOpen.value = true;
}

async function submitForm() {
  if (!form.title.trim()) {
    formError.value = t("s04.tasks.backlog.fieldTitleRequired");
    return;
  }
  acting.value = true;
  formError.value = "";
  try {
    if (editing.value) {
      await updateTask(editing.value.id, {
        revision: editing.value.revision,
        title: form.title.trim(),
        description: form.description.trim() || null,
        status: form.status,
        priority: form.priority,
        assignee_id: form.assignee.trim() || null
      });
    } else {
      await createTask({
        project_id: projectId.value,
        title: form.title.trim(),
        description: form.description.trim() || null,
        priority: form.priority,
        assignee_id: form.assignee.trim() || null
      });
    }
    formOpen.value = false;
    actionError.value = "";
    actionNotice.value = t("s04.tasks.backlog.saved");
    await load();
  } catch (error) {
    formError.value = describeError(error);
  } finally {
    acting.value = false;
  }
}

// ─── 依赖关系 ──────────────────────────────────────────────────────────

const depTask = ref<TaskRowModel | null>(null);
const depGraph = ref<TaskDependencyGraph | null>(null);
const depLoading = ref(false);
const depInput = ref("");
const depError = ref("");

const depOpen = computed(() => depTask.value !== null);

async function openDependencies(task: TaskRowModel) {
  depTask.value = task;
  depInput.value = "";
  depError.value = "";
  await reloadDependencies();
}

async function reloadDependencies() {
  if (!depTask.value) return;
  depLoading.value = true;
  try {
    depGraph.value = await listTaskDependencies(depTask.value.id);
  } catch (error) {
    depError.value = describeError(error);
  } finally {
    depLoading.value = false;
  }
}

async function submitDependency() {
  if (!depTask.value) return;
  const target = depInput.value.trim();
  if (!target) return;
  if (target === depTask.value.id) {
    depError.value = t("s04.tasks.backlog.dependencySelf");
    return;
  }
  depError.value = "";
  try {
    await addTaskDependency(depTask.value.id, target);
    depInput.value = "";
    await reloadDependencies();
  } catch (error) {
    depError.value = describeError(error);
  }
}

async function dropDependency(dependsOnTaskId: string) {
  if (!depTask.value) return;
  depError.value = "";
  try {
    await removeTaskDependency(depTask.value.id, dependsOnTaskId);
    await reloadDependencies();
  } catch (error) {
    depError.value = describeError(error);
  }
}

function depEdges(edges: TaskDependencyEdge[] | undefined) {
  return edges ?? [];
}

// ─── 看板拖拽（具体状态列才可落；聚合列不是单一状态）──────────────────

const draggingId = ref("");

function laneAcceptsDrop(laneKey: string): boolean {
  return ["draft", "ready", "blocked"].includes(laneKey);
}

async function onDropToLane(laneStatus: string) {
  const id = draggingId.value;
  draggingId.value = "";
  if (!id) return;
  const task = rows.value.find(item => item.id === id);
  if (!task || task.status === laneStatus) return;
  actionError.value = "";
  try {
    await updateTask(task.id, {
      revision: task.revision,
      title: task.title,
      description: task.description,
      status: laneStatus,
      priority: task.priority,
      assignee_id: task.assigneeId
    });
    await load();
  } catch (error) {
    actionError.value = describeError(error);
  }
}

function openDetail(task: TaskRowModel) {
  void router.push(`/project/tasks/${task.id}`);
}

const totalPages = computed(() =>
  Math.max(1, Math.ceil(total.value / PAGE_SIZE))
);

function gotoPage(next: number) {
  if (next < 1 || next > totalPages.value) return;
  page.value = next;
  void load();
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('shell.app.side.item.taskManagement') }]"
    :title="t('s04.tasks.page.backlogTitle')"
    :description="t('s04.tasks.page.backlogDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="backlog-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s04.tasks.common.refresh") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        data-testid="backlog-create"
        @click="openCreate"
      >
        <Plus :size="14" />
        {{ t("s04.tasks.common.createTask") }}
      </Button>
    </template>

    <TasksFrame />

    <Alert
      v-if="actionNotice"
      variant="success"
      :title="actionNotice"
      data-testid="backlog-notice"
    />
    <Alert
      v-if="actionError"
      variant="danger"
      :title="t('s04.tasks.common.actionFailed')"
      data-testid="backlog-action-error"
    >
      {{ actionError }}
    </Alert>
    <Alert
      v-if="loadError"
      variant="danger"
      :title="t('s04.tasks.common.errorTitle')"
    >
      {{ t("s04.tasks.overview.loadFailed") }}
    </Alert>

    <div class="bl-toolbar">
      <Select
        v-model="statusFilter"
        :options="statusOptions"
        :placeholder="t('s04.tasks.common.statusLabel')"
        data-testid="backlog-filter-status"
      />
      <Select
        v-model="priorityFilter"
        :options="priorityOptions"
        :placeholder="t('s04.tasks.common.priorityLabel')"
        data-testid="backlog-filter-priority"
      />
      <Input
        v-model="searchText"
        :placeholder="t('s04.tasks.common.searchPlaceholder')"
        data-testid="backlog-search"
      />
      <label class="bl-toolbar__check">
        <input
          v-model="unassignedOnly"
          type="checkbox"
          data-testid="backlog-filter-unassigned"
        />
        {{ t("s04.tasks.common.unassigned") }}
      </label>
      <Button v-if="hasFilters" variant="ghost" size="sm" @click="clearFilters">
        {{ t("s04.tasks.common.clearFilters") }}
      </Button>
      <div class="bl-toolbar__spacer" />
      <div class="bl-views" role="tablist">
        <button
          type="button"
          class="bl-views__btn"
          :class="{ 'is-active': viewMode === 'table' }"
          data-testid="backlog-view-table"
          @click="viewMode = 'table'"
        >
          <List :size="14" />
          {{ t("s04.tasks.backlog.viewTable") }}
        </button>
        <button
          type="button"
          class="bl-views__btn"
          :class="{ 'is-active': viewMode === 'kanban' }"
          data-testid="backlog-view-kanban"
          @click="viewMode = 'kanban'"
        >
          <Columns3 :size="14" />
          {{ t("s04.tasks.backlog.viewKanban") }}
        </button>
      </div>
    </div>

    <div v-if="selected.size > 0" class="bl-bulk" data-testid="backlog-bulk">
      <span>{{
        t("s04.tasks.common.selectedCount", { count: selected.size })
      }}</span>
      <Button
        variant="primary"
        size="sm"
        :disabled="markReadyDisabled"
        data-testid="backlog-mark-ready"
        @click="markReadyOpen = true"
      >
        {{ t("s04.tasks.backlog.markReady") }} ({{ selectedDrafts.length }})
      </Button>
      <Button variant="ghost" size="sm" @click="clearSelection">
        {{ t("s04.tasks.breakdown.clearSelection") }}
      </Button>
    </div>

    <div v-if="loading" class="bl-loading">
      {{ t("s04.tasks.common.loading") }}
    </div>

    <template v-else>
      <EmptyState
        v-if="rows.length === 0"
        :title="t('s04.tasks.common.empty')"
        :description="t('s04.tasks.common.emptyHint')"
      />
      <EmptyState
        v-else-if="filteredRows.length === 0"
        :title="t('s04.tasks.common.empty')"
        size="sm"
      />

      <table v-else-if="viewMode === 'table'" class="bl-table">
        <thead>
          <tr>
            <th class="bl-table__check-col" />
            <th>{{ t("s04.tasks.common.taskLabel") }}</th>
            <th>{{ t("s04.tasks.overview.execKindLabel") }}</th>
            <th>{{ t("s04.tasks.common.statusLabel") }}</th>
            <th>{{ t("s04.tasks.common.priorityLabel") }}</th>
            <th>{{ t("s04.tasks.common.assigneeLabel") }}</th>
            <th>{{ t("s04.tasks.backlog.colMilestone") }}</th>
            <th>{{ t("s04.tasks.common.updatedLabel") }}</th>
            <th class="bl-table__ops-col" />
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="task in filteredRows"
            :key="task.id"
            :data-testid="`bl-row-${shortId(task.id)}`"
          >
            <td>
              <input
                type="checkbox"
                :checked="selected.has(task.id)"
                :aria-label="task.title"
                :data-testid="`bl-check-${shortId(task.id)}`"
                @change="onRowCheck(task, $event)"
              />
            </td>
            <td class="bl-table__task">
              <button
                type="button"
                class="bl-table__link"
                @click="openDetail(task)"
              >
                <span class="bl-table__id">{{ task.code }}</span>
                <span>{{ task.title }}</span>
              </button>
            </td>
            <td>
              {{
                t(
                  `s04.tasks.overview.execKind${executionKind(task).charAt(0).toUpperCase()}${executionKind(task).slice(1)}`
                )
              }}
            </td>
            <td><TaskStatusChip :status="task.status" /></td>
            <td><TaskPriorityChip :priority="task.priority" /></td>
            <td>
              {{
                task.assigneeId
                  ? shortId(task.assigneeId)
                  : t("s04.tasks.common.unassigned")
              }}
            </td>
            <td>
              {{
                task.milestoneId
                  ? shortId(task.milestoneId)
                  : t("s04.tasks.common.notAvailable")
              }}
            </td>
            <td>{{ formatDateTime(task.updatedAt) }}</td>
            <td class="bl-table__ops">
              <Button variant="ghost" size="sm" @click="openEdit(task)">
                <Pencil :size="13" />
                {{ t("s04.tasks.backlog.editTask") }}
              </Button>
              <Button variant="ghost" size="sm" @click="openDependencies(task)">
                <GitBranch :size="13" />
                {{ t("s04.tasks.backlog.manageDependencies") }}
              </Button>
            </td>
          </tr>
        </tbody>
      </table>

      <div v-else class="bl-kanban">
        <p class="bl-kanban__hint">{{ t("s04.tasks.backlog.dragHint") }}</p>
        <div class="bl-kanban__board">
          <section
            v-for="lane in KANBAN_LANES"
            :key="lane.key"
            class="bl-lane"
            :data-testid="`bl-lane-${lane.key}`"
            @dragover="
              laneAcceptsDrop(lane.key) ? $event.preventDefault() : null
            "
            @drop="
              laneAcceptsDrop(lane.key)
                ? onDropToLane(lane.statuses[0] ?? '')
                : null
            "
          >
            <header class="bl-lane__head">
              <template v-if="lane.key === 'active' || lane.key === 'closed'">
                {{
                  t(
                    `s04.tasks.backlog.kanbanLane${lane.key === "active" ? "Active" : "Closed"}`
                  )
                }}
              </template>
              <template v-else>
                {{ t(`s04.tasks.status.${lane.statuses[0] ?? lane.key}`) }}
              </template>
              <span class="bl-lane__count">
                {{
                  rows.filter(task => lane.statuses.includes(task.status))
                    .length
                }}
              </span>
            </header>
            <article
              v-for="task in filteredRows.filter(item =>
                lane.statuses.includes(item.status)
              )"
              :key="task.id"
              class="bl-card"
              draggable="true"
              :data-testid="`bl-card-${shortId(task.id)}`"
              @dragstart="draggingId = task.id"
            >
              <p class="bl-card__title">
                <span class="bl-table__id">{{ shortId(task.id) }}</span>
                {{ task.title }}
              </p>
              <footer class="bl-card__meta">
                <TaskPriorityChip :priority="task.priority" />
                <span
                  v-if="lane.key === 'active' || lane.key === 'closed'"
                  class="bl-card__status"
                >
                  <TaskStatusChip :status="task.status" />
                </span>
              </footer>
            </article>
          </section>
        </div>
      </div>

      <div v-if="totalPages > 1" class="bl-pager">
        <Button
          variant="ghost"
          size="sm"
          :disabled="page <= 1"
          :aria-label="t('s04.tasks.common.prevPage')"
          @click="gotoPage(page - 1)"
        >
          <ChevronLeft :size="14" aria-hidden="true" />
        </Button>
        <span class="bl-pager__label">{{ page }} / {{ totalPages }}</span>
        <Button
          variant="ghost"
          size="sm"
          :disabled="page >= totalPages"
          :aria-label="t('s04.tasks.common.nextPage')"
          @click="gotoPage(page + 1)"
        >
          <ChevronRight :size="14" aria-hidden="true" />
        </Button>
      </div>
    </template>

    <Dialog
      :open="formOpen"
      :title="
        editing
          ? t('s04.tasks.backlog.editTitle')
          : t('s04.tasks.backlog.createTitle')
      "
      size="md"
      @update:open="formOpen = $event"
    >
      <div class="bl-form">
        <label class="bl-form__field">
          <span>{{ t("s04.tasks.backlog.fieldTitle") }}</span>
          <Input v-model="form.title" data-testid="backlog-form-title" />
        </label>
        <label class="bl-form__field">
          <span>{{ t("s04.tasks.backlog.fieldDesc") }}</span>
          <Textarea
            v-model="form.description"
            :rows="3"
            data-testid="backlog-form-desc"
          />
        </label>
        <div v-if="editing" class="bl-form__row">
          <label class="bl-form__field">
            <span>{{ t("s04.tasks.backlog.fieldStatus") }}</span>
            <Select
              v-model="form.status"
              :options="statusFormOptions"
              data-testid="backlog-form-status"
            />
          </label>
        </div>
        <div class="bl-form__row">
          <label class="bl-form__field">
            <span>{{ t("s04.tasks.backlog.fieldPriority") }}</span>
            <Select
              v-model="form.priority"
              :options="priorityFormOptions"
              data-testid="backlog-form-priority"
            />
          </label>
          <label class="bl-form__field">
            <span
              >{{ t("s04.tasks.backlog.fieldAssignee") }}（{{
                t("s04.tasks.common.optional")
              }}）</span
            >
            <Input
              v-model="form.assignee"
              data-testid="backlog-form-assignee"
            />
          </label>
        </div>
        <p
          v-if="formError"
          class="bl-form__error"
          data-testid="backlog-form-error"
        >
          {{ formError }}
        </p>
      </div>
      <template #footer>
        <Button variant="ghost" size="sm" @click="formOpen = false">
          {{ t("s04.tasks.common.cancel") }}
        </Button>
        <Button
          variant="primary"
          size="sm"
          :disabled="acting"
          data-testid="backlog-form-submit"
          @click="submitForm"
        >
          {{
            editing
              ? t("s04.tasks.backlog.submitUpdate")
              : t("s04.tasks.backlog.submitCreate")
          }}
        </Button>
      </template>
    </Dialog>

    <Dialog
      :open="depOpen"
      :title="t('s04.tasks.backlog.dependenciesTitle')"
      size="md"
      @update:open="depTask = null"
    >
      <div class="bl-deps">
        <p v-if="depLoading" class="bl-loading">
          {{ t("s04.tasks.common.loading") }}
        </p>
        <template v-else>
          <div class="bl-deps__group">
            <h4>{{ t("s04.tasks.backlog.dependsOn") }}</h4>
            <p
              v-if="depEdges(depGraph?.upstream).length === 0"
              class="bl-deps__empty"
            >
              {{ t("s04.tasks.backlog.noDependencies") }}
            </p>
            <ul v-else>
              <li v-for="edge in depEdges(depGraph?.upstream)" :key="edge.id">
                <span class="bl-table__id">{{ shortId(edge.id) }}</span>
                <span class="bl-deps__title">{{ edge.title }}</span>
                <TaskStatusChip :status="edge.status" />
                <button
                  type="button"
                  class="bl-deps__remove"
                  :data-testid="`bl-dep-remove-${shortId(edge.id)}`"
                  @click="dropDependency(edge.id)"
                >
                  {{ t("s04.tasks.backlog.dependencyRemove") }}
                </button>
              </li>
            </ul>
          </div>
          <div class="bl-deps__group">
            <h4>{{ t("s04.tasks.backlog.blocks") }}</h4>
            <p
              v-if="depEdges(depGraph?.downstream).length === 0"
              class="bl-deps__empty"
            >
              {{ t("s04.tasks.backlog.noDependencies") }}
            </p>
            <ul v-else>
              <li v-for="edge in depEdges(depGraph?.downstream)" :key="edge.id">
                <span class="bl-table__id">{{ shortId(edge.id) }}</span>
                <span class="bl-deps__title">{{ edge.title }}</span>
                <TaskStatusChip :status="edge.status" />
              </li>
            </ul>
          </div>
          <div class="bl-deps__add">
            <Input
              v-model="depInput"
              :placeholder="t('s04.tasks.backlog.dependencyPlaceholder')"
              data-testid="bl-dep-input"
            />
            <Button
              variant="secondary"
              size="sm"
              data-testid="bl-dep-add"
              @click="submitDependency"
            >
              {{ t("s04.tasks.backlog.dependencyAdd") }}
            </Button>
          </div>
          <p v-if="depError" class="bl-form__error" data-testid="bl-dep-error">
            {{ depError }}
          </p>
        </template>
      </div>
    </Dialog>

    <ConfirmationDialog
      :open="markReadyOpen"
      :title="t('s04.tasks.backlog.markReadyConfirmTitle')"
      :description="
        t('s04.tasks.backlog.markReadyConfirmDesc', {
          count: selectedDrafts.length
        })
      "
      :confirm-label="t('s04.tasks.backlog.markReady')"
      :cancel-label="t('s04.tasks.common.cancel')"
      :disabled="acting"
      @confirm="confirmMarkReady"
      @cancel="markReadyOpen = false"
    />
  </PageFrame>
</template>

<style scoped>
.bl-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.bl-toolbar__check {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.bl-toolbar__spacer {
  flex: 1;
}

.bl-views {
  display: inline-flex;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.bl-views__btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  height: var(--size-control-sm);
  border: none;
  background: var(--ws-surface);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.bl-views__btn.is-active {
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
}

.bl-bulk {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border: var(--border-1) solid var(--ws-accent);
  border-radius: var(--radius-sm);
  background: var(--ws-color-accent-surface);
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.bl-loading {
  padding: var(--space-8) 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  text-align: center;
}

.bl-table {
  width: 100%;
  border-collapse: collapse;
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
}

.bl-table th {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  text-align: left;
  white-space: nowrap;
}

.bl-table td {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-variant-numeric: tabular-nums;
  vertical-align: middle;
}

.bl-table__check-col {
  width: var(--space-8);
}

.bl-table__ops-col {
  width: auto;
}

.bl-table__ops {
  display: flex;
  gap: var(--space-2);
  white-space: nowrap;
}

.bl-table__link {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-3);
  border: none;
  background: none;
  padding: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  cursor: pointer;
  text-align: left;
}

.bl-table__link:hover {
  color: var(--ws-accent);
}

.bl-table__id {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

.bl-kanban__hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.bl-kanban__board {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: var(--space-3);
}

.bl-lane {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  min-height: var(--space-16);
}

.bl-lane__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.bl-lane__count {
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.bl-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  cursor: grab;
}

.bl-card:hover {
  border-color: var(--ws-accent);
}

.bl-card__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.bl-card__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.bl-pager {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
}

.bl-pager__label {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.bl-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.bl-form__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1;
}

.bl-form__field > span {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.bl-form__row {
  display: flex;
  gap: var(--space-4);
}

.bl-form__error {
  margin: 0;
  color: var(--ws-color-danger-text);
  font-size: var(--font-size-caption);
}

.bl-deps {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.bl-deps__group h4 {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.bl-deps__group ul {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.bl-deps__group li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.bl-deps__title {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.bl-deps__empty {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.bl-deps__remove {
  border: none;
  background: none;
  padding: 0;
  color: var(--ws-color-danger-text);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.bl-deps__add {
  display: flex;
  gap: var(--space-2);
}

.bl-deps__add > :first-child {
  flex: 1;
}

@media (max-width: 1100px) {
  .bl-kanban__board {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
