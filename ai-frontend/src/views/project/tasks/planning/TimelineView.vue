<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { CalendarCheck, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import { getDependencyGraph, setTaskSchedule } from "@/api/task-plans";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import { useDesignProject } from "../../design/use-design-project";
import PlanningViewTabs from "./PlanningViewTabs.vue";
import {
  diffDays,
  shiftTimestamp,
  toDateKey,
  toTimelineView,
  type TimelineRowView,
  type TimelineViewModel
} from "./timeline-adapter";

/**
 * 执行规划 · 时间线页根（S04-UI-06，接真）。
 *
 * 数据 = pm-api REST：
 * - `GET /v1/pm/projects/{project_id}/dependency-graph` —— 节点自带
 *   planned_start_at / planned_end_at / estimate_hours，边即 FS 依赖；
 * - `PATCH /v1/pm/tasks/{task_id}/schedule` —— **写生效排期**（三值全量
 *   替换：planned_start_at / planned_end_at / estimate_hours）。
 *
 * 纪律：
 * - **未排期**任务进未排期托盘（不静默丢弃），拖入画布即写入开始日期；
 * - 拖动任务条 = 移动生效排期（下游顺延一并写入）；拖右手柄 = 改终点；
 * - 里程碑 / 特性分组 / 负责人 后端无来源 → 分组改为**仓库**（节点真实
 *   字段），负载分布同口径（按工时估算）。
 */
const { t } = useI18n();
const { projectId, hasProject } = useDesignProject();

const model = ref<TimelineViewModel | null>(null);
const loading = ref(true);
const loadError = ref(false);
const saving = ref(false);
const notice = ref("");
const errorNotice = ref("");

async function load() {
  if (!hasProject.value) {
    model.value = null;
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    model.value = toTimelineView(await getDependencyGraph(projectId.value));
  } catch {
    loadError.value = true;
    model.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const rows = computed<TimelineRowView[]>(() => model.value?.rows ?? []);
const groups = computed(() => model.value?.groups ?? []);
const edges = computed(() => model.value?.edges ?? []);
const gridStart = computed(() => model.value?.gridStart ?? "");
const gridDays = computed(() => model.value?.gridDays ?? 42);

const scale = ref<"day" | "week" | "month">("week");
const pxPerDay = computed(() =>
  scale.value === "day" ? 64 : scale.value === "week" ? 18 : 6
);
const canvasWidth = computed(() => gridDays.value * pxPerDay.value);

const collapsed = ref<Set<string>>(new Set());

function toggleGroup(id: string) {
  const next = new Set(collapsed.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  collapsed.value = next;
}

const showDeps = ref(true);

interface RenderRow {
  kind: "group" | "task";
  groupId: string;
  groupName?: string;
  row?: TimelineRowView;
}

const renderRows = computed<RenderRow[]>(() => {
  const out: RenderRow[] = [];
  for (const group of groups.value) {
    out.push({
      kind: "group",
      groupId: group.id,
      groupName: group.repo
    });
    if (collapsed.value.has(group.id)) continue;
    for (const row of group.rows) {
      out.push({ kind: "task", groupId: group.id, row });
    }
  }
  return out;
});

const rowIndexByCode = computed(() => {
  const map = new Map<string, number>();
  renderRows.value.forEach((item, index) => {
    if (item.kind === "task" && item.row) map.set(item.row.code, index);
  });
  return map;
});

const scheduledRows = computed(() => rows.value.filter(row => row.start));
const unscheduledRows = computed(() => rows.value.filter(row => !row.start));

const weekCount = computed(() => Math.ceil(gridDays.value / 7));

function addDays(iso: string, days: number): string {
  const time = new Date(`${iso}T00:00:00Z`).getTime();
  if (Number.isNaN(time)) return iso;
  return new Date(time + days * 86_400_000).toISOString().slice(0, 10);
}

function fmtDay(iso: string): string {
  return iso.slice(5).replace("-", "/");
}

const weekLabels = computed(() =>
  Array.from({ length: weekCount.value }, (_, index) => {
    const start = addDays(gridStart.value, index * 7);
    return `W${index + 1} · ${fmtDay(start)}`;
  })
);

const dayLabels = computed(() =>
  Array.from({ length: gridDays.value }, (_, index) =>
    fmtDay(addDays(gridStart.value, index))
  )
);

// ─── 任务条几何 ────────────────────────────────────────────────────────

/** 任务条跨度（天）：起止日期之差 + 1；无终点按 1 天。 */
function spanDays(row: TimelineRowView): number {
  if (!row.start) return 1;
  const start = toDateKey(row.start);
  const end = row.end ? toDateKey(row.end) : start;
  return Math.max(1, diffDays(start, end) + 1);
}

function barLeft(row: TimelineRowView): number {
  if (!row.start) return 0;
  return diffDays(gridStart.value, toDateKey(row.start)) * pxPerDay.value;
}

function barWidth(row: TimelineRowView): number {
  return Math.max(spanDays(row) * pxPerDay.value - pxPerDay.value * 0.15, 6);
}

// ─── 依赖连线（FS） ───────────────────────────────────────────────────

const ROW_H = 36;

interface EdgePath {
  key: string;
  d: string;
}

const depPaths = computed<EdgePath[]>(() => {
  if (!showDeps.value) return [];
  const out: EdgePath[] = [];
  for (const dep of edges.value) {
    const fromRow = rows.value.find(row => row.id === dep.from);
    const toRow = rows.value.find(row => row.id === dep.to);
    const fromIndex = rowIndexByCode.value.get(fromRow?.code ?? "");
    const toIndex = rowIndexByCode.value.get(toRow?.code ?? "");
    if (!fromRow?.start || !toRow?.start) continue;
    if (fromIndex === undefined || toIndex === undefined) continue;
    const x1 = barLeft(fromRow) + barWidth(fromRow);
    const y1 = fromIndex * ROW_H + ROW_H / 2;
    const x2 = barLeft(toRow);
    const y2 = toIndex * ROW_H + ROW_H / 2;
    const midX = x1 + Math.max((x2 - x1) / 2, 8);
    out.push({
      key: `${dep.from}->${dep.to}`,
      d:
        y1 === y2
          ? `M ${x1} ${y1} H ${x2}`
          : `M ${x1} ${y1} H ${midX} V ${y2} H ${x2}`
    });
  }
  return out;
});

// ─── 拖拽（移动 / 改终点 / 未排期拖入） ───────────────────────────────

const drag = ref<{
  id: string;
  kind: "move" | "resize";
  startX: number;
} | null>(null);
const preview = ref<{ id: string; deltaDays: number } | null>(null);
const draggingUnscheduled = ref("");

function descendantsOf(code: string, seen = new Set<string>()): Set<string> {
  for (const dep of edges.value.filter(item => item.from === code)) {
    if (!seen.has(dep.to)) {
      seen.add(dep.to);
      descendantsOf(dep.to, seen);
    }
  }
  return seen;
}

function onBarPointerDown(
  row: TimelineRowView,
  kind: "move" | "resize",
  event: PointerEvent
) {
  if (!row.start) return;
  drag.value = { id: row.id, kind, startX: event.clientX };
  (event.target as HTMLElement).setPointerCapture?.(event.pointerId);
}

function onBarPointerMove(event: PointerEvent) {
  if (!drag.value) return;
  const deltaDays = Math.round(
    (event.clientX - drag.value.startX) / pxPerDay.value
  );
  if (deltaDays === 0) {
    preview.value = null;
    return;
  }
  preview.value = { id: drag.value.id, deltaDays };
}

const pendingMove = ref<{
  id: string;
  deltaDays: number;
  affected: string[];
} | null>(null);
const confirmOpen = ref(false);

const pendingMoveTarget = computed(
  () => rows.value.find(row => row.id === pendingMove.value?.id) ?? null
);

function onBarPointerUp() {
  const dragInfo = drag.value;
  const move = preview.value;
  drag.value = null;
  preview.value = null;
  if (!dragInfo || !move) return;
  if (dragInfo.kind === "resize") {
    void applyResize(dragInfo.id, move.deltaDays);
    return;
  }
  const affected = [...descendantsOf(dragInfo.id)].filter(id => {
    const row = rows.value.find(item => item.id === id);
    return Boolean(row?.start);
  });
  pendingMove.value = { id: dragInfo.id, deltaDays: move.deltaDays, affected };
  confirmOpen.value = true;
}

async function applyMove() {
  const move = pendingMove.value;
  confirmOpen.value = false;
  pendingMove.value = null;
  if (!move) return;
  const targets = [move.id, ...move.affected];
  saving.value = true;
  notice.value = "";
  errorNotice.value = "";
  try {
    for (const id of targets) {
      const row = rows.value.find(item => item.id === id);
      if (!row?.start) continue;
      await setTaskSchedule(id, {
        planned_start_at: shiftTimestamp(row.start, move.deltaDays),
        planned_end_at: row.end
          ? shiftTimestamp(row.end, move.deltaDays)
          : null,
        estimate_hours: row.effortHours
      });
    }
    notice.value = t("s04.tasks.timeline.scheduleSaved", {
      count: targets.length
    });
    await load();
  } catch {
    errorNotice.value = t("s04.tasks.timeline.scheduleFailed");
    await load();
  } finally {
    saving.value = false;
  }
}

function cancelMove() {
  confirmOpen.value = false;
  pendingMove.value = null;
}

async function applyResize(id: string, deltaDays: number) {
  const row = rows.value.find(item => item.id === id);
  if (!row?.start) return;
  const base = row.end ?? row.start;
  const nextEnd = shiftTimestamp(base, deltaDays);
  if (new Date(nextEnd).getTime() < new Date(row.start).getTime()) return;
  saving.value = true;
  notice.value = "";
  errorNotice.value = "";
  try {
    await setTaskSchedule(id, {
      planned_start_at: row.start,
      planned_end_at: nextEnd,
      estimate_hours: row.effortHours
    });
    notice.value = t("s04.tasks.timeline.scheduleSaved", { count: 1 });
    await load();
  } catch {
    errorNotice.value = t("s04.tasks.timeline.scheduleFailed");
    await load();
  } finally {
    saving.value = false;
  }
}

async function onDropSchedule(row: TimelineRowView, event: DragEvent) {
  const id =
    draggingUnscheduled.value ||
    event.dataTransfer?.getData("text/plain") ||
    "";
  if (id !== row.id) return;
  const target = event.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  const offset = event.clientX - rect.left;
  const day = Math.max(
    0,
    Math.min(gridDays.value - 1, Math.round(offset / pxPerDay.value))
  );
  const start = `${addDays(gridStart.value, day)}T00:00:00.000Z`;
  draggingUnscheduled.value = "";
  saving.value = true;
  notice.value = "";
  errorNotice.value = "";
  try {
    await setTaskSchedule(id, {
      planned_start_at: start,
      planned_end_at: null,
      estimate_hours: row.effortHours
    });
    notice.value = t("s04.tasks.timeline.scheduleSaved", { count: 1 });
    await load();
  } catch {
    errorNotice.value = t("s04.tasks.timeline.scheduleFailed");
    await load();
  } finally {
    saving.value = false;
  }
}

// ─── 负载分布（按周 × 仓库） ──────────────────────────────────────────

const repos = computed(() =>
  groups.value.map(group => group.repo).filter(repo => repo.length > 0)
);

/** 某仓库某周内的工时估算（小时，按任务跨度均摊）。 */
function hoursInWeek(repo: string, week: number): number {
  let hours = 0;
  for (const row of rows.value) {
    if (row.repo !== repo || !row.start || !row.effortHours) continue;
    const days = spanDays(row);
    const perDay = row.effortHours / days;
    const startIdx = diffDays(gridStart.value, toDateKey(row.start));
    const overlapStart = Math.max(startIdx, week * 7);
    const overlapEnd = Math.min(startIdx + days, (week + 1) * 7);
    hours += Math.max(0, overlapEnd - overlapStart) * perDay;
  }
  return Math.round(hours * 10) / 10;
}

const maxWeekLoad = computed(() => {
  let max = 1;
  for (const repo of repos.value) {
    for (let week = 0; week < weekCount.value; week += 1) {
      max = Math.max(max, hoursInWeek(repo, week));
    }
  }
  return max;
});

// ─── 排期校验 ─────────────────────────────────────────────────────────

interface ScheduleIssue {
  key: string;
  text: string;
  tone: "warning" | "danger";
}

const scheduleIssues = ref<ScheduleIssue[] | null>(null);

function validateSchedule() {
  const issues: ScheduleIssue[] = [];
  if (unscheduledRows.value.length > 0) {
    issues.push({
      key: "unscheduled",
      text: t("s04.tasks.timeline.unscheduledIssue", {
        count: unscheduledRows.value.length
      }),
      tone: "warning"
    });
  }
  for (const dep of edges.value) {
    const fromRow = rows.value.find(row => row.id === dep.from);
    const toRow = rows.value.find(row => row.id === dep.to);
    if (!fromRow?.start || !toRow?.start || !fromRow.end) continue;
    if (new Date(toRow.start).getTime() < new Date(fromRow.end).getTime()) {
      issues.push({
        key: `order-${dep.from}-${dep.to}`,
        text: t("s04.tasks.timeline.badOrder", { code: toRow.code }),
        tone: "danger"
      });
    }
  }
  scheduleIssues.value = issues;
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('shell.app.side.item.taskManagement') }]"
    :title="t('s04.tasks.page.planningTitle')"
    :description="t('s04.tasks.page.planningDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="tl-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s04.tasks.common.refresh") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        data-testid="tl-validate"
        @click="validateSchedule"
      >
        <CalendarCheck :size="14" />
        {{ t("s04.tasks.planning.validateSchedule") }}
      </Button>
    </template>

    <TasksFrame />
    <PlanningViewTabs />

    <Alert
      v-if="loadError"
      variant="danger"
      :title="t('s04.tasks.common.errorTitle')"
    >
      {{ t("s04.tasks.planning.loadFailed") }}
    </Alert>

    <EmptyState
      v-else-if="!hasProject"
      :title="t('s04.tasks.common.noProjectTitle')"
      :description="t('s04.tasks.common.noProjectHint')"
    />

    <EmptyState
      v-else-if="!loading && rows.length === 0"
      :title="t('s04.tasks.common.empty')"
      :description="t('s04.tasks.planning.noTasksHint')"
    />

    <template v-else>
      <Alert
        v-if="notice"
        variant="success"
        :title="notice"
        data-testid="tl-notice"
      />
      <Alert
        v-if="errorNotice"
        variant="danger"
        :title="errorNotice"
        data-testid="tl-error"
      />

      <div class="tl-metrics">
        <span class="tl-metric"
          >{{ t("s04.tasks.planning.metricsScheduled") }}:
          {{ scheduledRows.length }}</span
        >
        <span class="tl-metric"
          >{{ t("s04.tasks.planning.metricsUnscheduled") }}:
          {{ unscheduledRows.length }}</span
        >
        <span class="tl-metric"
          >{{ t("s04.tasks.planning.metricsCritical") }}:
          {{ model?.criticalCount ?? 0 }}</span
        >
      </div>

      <div class="tl-toolbar">
        <div class="tl-scales" role="tablist">
          <button
            v-for="option in ['day', 'week', 'month'] as const"
            :key="option"
            type="button"
            class="tl-scale"
            :class="{ 'is-active': scale === option }"
            :data-testid="`tl-scale-${option}`"
            @click="scale = option"
          >
            {{
              t(
                `s04.tasks.timeline.scale${option.charAt(0).toUpperCase()}${option.slice(1)}`
              )
            }}
          </button>
        </div>
        <label class="tl-check">
          <input v-model="showDeps" type="checkbox" />
          {{ t("s04.tasks.timeline.showDeps") }}
        </label>
        <div class="tl-toolbar__spacer" />
      </div>

      <p class="tl-hint">{{ t("s04.tasks.timeline.dragHint") }}</p>

      <div class="tl-board" data-testid="tl-board">
        <div class="tl-board__scroll">
          <div
            class="tl-board__inner"
            :style="{ minWidth: `${240 + canvasWidth}px` }"
          >
            <div class="tl-header-row">
              <div class="tl-left-cell tl-left-cell--head">
                {{ t("s04.tasks.common.taskLabel") }}
              </div>
              <div
                class="tl-header-track"
                :style="{ width: `${canvasWidth}px` }"
              >
                <div v-if="scale === 'week'" class="tl-header-cells">
                  <span
                    v-for="label in weekLabels"
                    :key="label"
                    class="tl-header-cell"
                    :style="{ width: `${7 * pxPerDay}px` }"
                  >
                    {{ label }}
                  </span>
                </div>
                <div v-else-if="scale === 'day'" class="tl-header-cells">
                  <span
                    v-for="label in dayLabels"
                    :key="label"
                    class="tl-header-cell"
                    :style="{ width: `${pxPerDay}px` }"
                  >
                    {{ label }}
                  </span>
                </div>
                <div v-else class="tl-header-cells">
                  <span
                    class="tl-header-cell"
                    :style="{ width: `${canvasWidth}px` }"
                    >{{ gridStart.slice(0, 7) }}</span
                  >
                </div>
              </div>
            </div>

            <div
              v-for="(item, index) in renderRows"
              :key="`${item.groupId}-${index}`"
              class="tl-row"
              :class="{ 'tl-row--feature': item.kind === 'group' }"
            >
              <div
                class="tl-left-cell"
                :class="{ 'tl-left-cell--feature': item.kind === 'group' }"
              >
                <button
                  v-if="item.kind === 'group'"
                  type="button"
                  class="tl-feature"
                  :data-testid="`tl-group-${item.groupId}`"
                  @click="toggleGroup(item.groupId)"
                >
                  {{ collapsed.has(item.groupId) ? "▸" : "▾" }}
                  {{ item.groupName || t("s04.tasks.timeline.groupUnbound") }}
                </button>
                <template v-else-if="item.row">
                  <span class="tl-left-code">{{ item.row.code }}</span>
                  <span class="tl-left-title">{{ item.row.title }}</span>
                  <TaskPriorityChip :priority="item.row.priority" />
                </template>
              </div>
              <div
                v-if="item.kind === 'task' && item.row"
                class="tl-track"
                :class="{
                  'tl-track--droppable': draggingUnscheduled === item.row.code
                }"
                :style="{ width: `${canvasWidth}px` }"
                :data-testid="`tl-track-${item.row.code}`"
                @dragover.prevent
                @drop="onDropSchedule(item.row, $event)"
              >
                <template v-if="item.row.start">
                  <div
                    class="tl-bar"
                    :class="{
                      'is-critical': item.row.critical,
                      'is-blocked': item.row.blocked,
                      'is-preview': preview?.id === item.row.id
                    }"
                    :style="{
                      left: `${barLeft(item.row)}px`,
                      width: `${barWidth(item.row)}px`
                    }"
                    :data-testid="`tl-bar-${item.row.code}`"
                    :data-start="toDateKey(item.row.start)"
                    :data-end="item.row.end ? toDateKey(item.row.end) : ''"
                    :data-hours="item.row.effortHours ?? ''"
                    draggable="false"
                    @pointerdown="onBarPointerDown(item.row, 'move', $event)"
                    @pointermove="onBarPointerMove"
                    @pointerup="onBarPointerUp"
                  >
                    <span class="tl-bar__label">{{
                      item.row.effortHours !== null
                        ? `${item.row.effortHours}h`
                        : item.row.code
                    }}</span>
                    <span
                      class="tl-bar__handle"
                      @pointerdown.stop="
                        onBarPointerDown(item.row, 'resize', $event)
                      "
                    />
                  </div>
                </template>
                <span v-else class="tl-unscheduled-mark">{{
                  t("s04.tasks.planning.metricsUnscheduled")
                }}</span>
              </div>
              <div
                v-else-if="item.kind === 'group'"
                class="tl-track tl-track--feature"
              />
            </div>

            <svg
              v-if="showDeps"
              class="tl-deps"
              :width="canvasWidth"
              :height="renderRows.length * ROW_H"
            >
              <path
                v-for="path in depPaths"
                :key="path.key"
                :d="path.d"
                fill="none"
                class="tl-dep"
              />
            </svg>
          </div>
        </div>

        <aside
          v-if="unscheduledRows.length > 0"
          class="tl-tray"
          data-testid="tl-tray"
        >
          <h2>
            {{ t("s04.tasks.timeline.unscheduledTray") }} ({{
              unscheduledRows.length
            }})
          </h2>
          <ul>
            <li
              v-for="row in unscheduledRows"
              :key="row.id"
              draggable="true"
              :data-testid="`tl-tray-${row.code}`"
              @dragstart="draggingUnscheduled = row.code"
              @dragend="draggingUnscheduled = ''"
            >
              <span class="tl-left-code">{{ row.code }}</span>
              <span class="tl-tray__title">{{ row.title }}</span>
            </li>
          </ul>
        </aside>
      </div>

      <section
        v-if="scheduleIssues"
        class="tl-issues"
        data-testid="tl-validation"
      >
        <h2>{{ t("s04.tasks.planning.validationTitle") }}</h2>
        <EmptyState
          v-if="scheduleIssues.length === 0"
          :title="t('s04.tasks.planning.validationClean')"
          size="sm"
        />
        <ul v-else>
          <li
            v-for="issue in scheduleIssues"
            :key="issue.key"
            class="tl-issue"
            :class="`tl-issue--${issue.tone}`"
          >
            {{ issue.text }}
          </li>
        </ul>
      </section>

      <section class="tl-workload">
        <h2>{{ t("s04.tasks.timeline.workloadTitle") }}</h2>
        <p class="tl-hint">{{ t("s04.tasks.timeline.workloadHint") }}</p>
        <EmptyState
          v-if="repos.length === 0"
          :title="t('s04.tasks.timeline.workloadEmpty')"
          size="sm"
        />
        <div v-else class="tl-workload__grid">
          <div
            class="tl-workload__head"
            :style="{
              gridTemplateColumns: `var(--space-16) repeat(${weekCount}, minmax(0, 1fr))`
            }"
          >
            <span />
            <span v-for="label in weekLabels" :key="label">{{ label }}</span>
          </div>
          <div
            v-for="repo in repos"
            :key="repo"
            class="tl-workload__row"
            :style="{
              gridTemplateColumns: `var(--space-16) repeat(${weekCount}, minmax(0, 1fr))`
            }"
            :data-testid="`tl-load-${repo}`"
          >
            <span class="tl-workload__name">{{ repo }}</span>
            <span
              v-for="week in weekCount"
              :key="week"
              class="tl-workload__cell"
            >
              <i
                class="tl-workload__bar"
                :style="{
                  height: `${(hoursInWeek(repo, week - 1) / maxWeekLoad) * 100}%`
                }"
                :title="`${hoursInWeek(repo, week - 1)}h`"
              />
            </span>
          </div>
        </div>
      </section>
    </template>

    <ConfirmationDialog
      :open="confirmOpen"
      :title="t('s04.tasks.timeline.confirmMoveTitle')"
      :description="
        pendingMoveTarget
          ? t('s04.tasks.timeline.confirmMoveDesc', {
              code: pendingMoveTarget.code,
              delta:
                pendingMove && pendingMove.deltaDays > 0
                  ? `+${pendingMove.deltaDays}`
                  : `${pendingMove?.deltaDays ?? 0}`,
              count: pendingMove?.affected.length ?? 0
            })
          : ''
      "
      :confirm-label="t('s04.tasks.timeline.saveChanges')"
      :cancel-label="t('s04.tasks.common.cancel')"
      @confirm="applyMove"
      @cancel="cancelMove"
    />
  </PageFrame>
</template>

<style scoped>
.tl-metrics {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-4);
}

.tl-metric {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.tl-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.tl-toolbar__spacer {
  flex: 1;
}

.tl-scales {
  display: inline-flex;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.tl-scale {
  padding: 0 var(--space-3);
  height: var(--size-control-sm);
  border: none;
  background: var(--ws-surface);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.tl-scale.is-active {
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
}

.tl-check {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.tl-hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.tl-board {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.tl-board__scroll {
  overflow-x: auto;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  background: var(--ws-surface);
}

.tl-board__inner {
  position: relative;
}

.tl-header-row {
  display: flex;
  position: sticky;
  top: 0;
  background: var(--ws-surface);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.tl-left-cell {
  position: sticky;
  left: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: calc(var(--space-24) * 2.5);
  min-width: calc(var(--space-24) * 2.5);
  height: var(--ws-row-height);
  padding: 0 var(--space-3);
  background: var(--ws-surface);
  border-right: var(--border-1) solid var(--ws-border);
  overflow: hidden;
  white-space: nowrap;
}

.tl-left-cell--head {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.tl-left-cell--feature {
  background: var(--ws-elevated);
}

.tl-left-code {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.tl-left-title {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  overflow: hidden;
  text-overflow: ellipsis;
}

.tl-feature {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  border: none;
  background: none;
  padding: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  cursor: pointer;
}

.tl-header-track {
  position: relative;
}

.tl-header-cells {
  display: flex;
  height: var(--ws-row-height);
}

.tl-header-cell {
  display: inline-flex;
  align-items: center;
  padding: 0 var(--space-2);
  border-right: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  white-space: nowrap;
  overflow: hidden;
}

.tl-row {
  display: flex;
  border-bottom: var(--border-1) solid var(--ws-border);
}

.tl-row--feature .tl-track--feature {
  background: var(--ws-elevated);
  height: var(--ws-row-height);
}

.tl-track {
  position: relative;
  height: var(--ws-row-height);
}

.tl-track--droppable {
  outline: var(--size-focus-ring) dashed var(--ws-color-info);
}

.tl-bar {
  position: absolute;
  top: calc((var(--ws-row-height) - var(--space-4)) / 2);
  height: var(--space-4);
  display: flex;
  align-items: center;
  padding: 0 var(--space-2);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-text-disabled);
  border-radius: var(--radius-xs);
  cursor: grab;
  touch-action: none;
  user-select: none;
}

.tl-bar.is-critical {
  background: var(--ws-color-accent-surface);
  border-color: var(--ws-accent);
}

.tl-bar.is-blocked {
  border-color: var(--ws-color-danger);
  background-image: repeating-linear-gradient(
    45deg,
    transparent 0,
    transparent var(--space-1),
    var(--ws-color-danger-surface) var(--space-1),
    var(--ws-color-danger-surface) calc(var(--space-1) * 2)
  );
}

.tl-bar.is-preview {
  outline: var(--size-focus-ring) dashed var(--ws-color-info);
}

.tl-bar__label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-micro);
  white-space: nowrap;
  pointer-events: none;
}

.tl-bar__handle {
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: var(--space-2);
  cursor: ew-resize;
}

.tl-unscheduled-mark {
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
  padding: 0 var(--space-2);
}

.tl-deps {
  position: absolute;
  top: calc(var(--ws-row-height) + var(--space-4));
  left: calc(var(--space-24) * 2.5);
  pointer-events: none;
}

.tl-dep {
  stroke: var(--ws-text-disabled);
  stroke-width: var(--border-1);
}

.tl-tray {
  border: var(--border-1) dashed var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-3) var(--space-4);
}

.tl-tray h2 {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.tl-tray ul {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.tl-tray li {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  background: var(--ws-elevated);
  border-radius: var(--radius-sm);
  cursor: grab;
}

.tl-tray__title {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.tl-issues,
.tl-workload {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-4) var(--space-5);
}

.tl-issues h2,
.tl-workload h2 {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0 0 var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.tl-issues ul {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.tl-issue {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
}

.tl-issue--danger {
  border: var(--border-1) solid var(--ws-color-danger);
  color: var(--ws-color-danger);
}

.tl-issue--warning {
  border: var(--border-1) solid var(--ws-color-warning);
  color: var(--ws-color-warning);
}

.tl-workload__grid {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.tl-workload__head,
.tl-workload__row {
  display: grid;
  gap: var(--space-2);
  align-items: end;
}

.tl-workload__head span {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.tl-workload__name {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.tl-workload__cell {
  display: flex;
  align-items: flex-end;
  height: var(--space-8);
  background: var(--ws-canvas);
  border-radius: var(--radius-xs);
}

.tl-workload__bar {
  width: 100%;
  background: var(--ws-accent);
  border-radius: var(--radius-xs) var(--radius-xs) 0 0;
  display: block;
}

@media (max-width: 1100px) {
  .tl-left-cell {
    width: calc(var(--space-24) * 2);
    min-width: calc(var(--space-24) * 2);
  }
}
</style>
