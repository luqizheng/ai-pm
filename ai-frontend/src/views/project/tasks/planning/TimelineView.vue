<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { CalendarCheck, Sparkles } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Select from "@/components/ui/select/Select.vue";
import { PageFrame } from "@/components/shell";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import PlanningViewTabs from "./PlanningViewTabs.vue";
import {
  TIMELINE_DAYS,
  TIMELINE_START,
  timelineDeps,
  timelineFeatures,
  timelineMilestones,
  timelineRecs,
  timelineRows,
  type TimelineRowFixture
} from "./fixtures";

/**
 * 执行规划 · 时间线页根（S04-UI-06，mock 预览）。
 *
 * 任务域无 planned dates 字段（delivery.task 只有时间戳），本页以
 * fixtures 演示排期交互：双栏同步甘特（日/周/月比例尺）、拖拽移动/
 * 拉伸任务条（下游顺延预览 + 确认）、未排期任务拖入排期、FS 依赖连线、
 * 里程碑、按周负载分布、AI 排期建议（预览/采纳）、排期校验与规划版本
 * （保存快照 + 比较虚线）。排期日期是规划估算，与实际执行时间无关；
 * 全部改动仅存在于本页会话。
 */
const { t } = useI18n();

type Row = TimelineRowFixture;

const rows = ref<Row[]>(timelineRows());
const deps = timelineDeps();
const features = timelineFeatures();
const milestones = timelineMilestones();
const recs = ref(timelineRecs());
const recDecisions = ref<Record<string, "accepted" | "rejected">>({});

const scale = ref<"day" | "week" | "month">("week");
const pxPerDay = computed(() =>
  scale.value === "day" ? 64 : scale.value === "week" ? 18 : 6
);

const gridStartMs = new Date(TIMELINE_START).getTime();
const DAY_MS = 86_400_000;

function dayIndex(iso: string): number {
  return Math.round((new Date(iso).getTime() - gridStartMs) / DAY_MS);
}

function addDays(iso: string, days: number): string {
  return new Date(new Date(iso).getTime() + days * DAY_MS)
    .toISOString()
    .slice(0, 10);
}

function fmtDay(iso: string): string {
  return iso.slice(5).replace("-", "/");
}

const canvasWidth = computed(() => TIMELINE_DAYS * pxPerDay.value);

const collapsed = ref<Set<string>>(new Set());

function toggleFeature(id: string) {
  const next = new Set(collapsed.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  collapsed.value = next;
}

const showDeps = ref(true);
const showMilestones = ref(true);

interface RenderRow {
  kind: "feature" | "task" | "gap";
  featureId: string;
  featureName?: string;
  row?: Row;
}

const renderRows = computed<RenderRow[]>(() => {
  const out: RenderRow[] = [];
  for (const feature of features) {
    out.push({
      kind: "feature",
      featureId: feature.id,
      featureName: feature.name
    });
    if (collapsed.value.has(feature.id)) continue;
    for (const row of rows.value.filter(
      item => item.featureId === feature.id
    )) {
      out.push({ kind: "task", featureId: feature.id, row });
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

const weekCount = computed(() => Math.ceil(TIMELINE_DAYS / 7));

const weekLabels = computed(() =>
  Array.from({ length: weekCount.value }, (_, index) => {
    const start = addDays(TIMELINE_START, index * 7);
    return `W${index + 1} · ${fmtDay(start)}`;
  })
);

const dayLabels = computed(() =>
  Array.from({ length: TIMELINE_DAYS }, (_, index) =>
    fmtDay(addDays(TIMELINE_START, index))
  )
);

// ─── 任务条几何 ────────────────────────────────────────────────────────

function barLeft(row: Row): number {
  return (row.start ? dayIndex(row.start) : 0) * pxPerDay.value;
}

function barWidth(row: Row): number {
  return Math.max(row.effortDays * pxPerDay.value - pxPerDay.value * 0.15, 6);
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
  for (const dep of deps) {
    const fromRow = rows.value.find(row => row.code === dep.from);
    const toRow = rows.value.find(row => row.code === dep.to);
    const fromIndex = rowIndexByCode.value.get(dep.from);
    const toIndex = rowIndexByCode.value.get(dep.to);
    if (
      !fromRow?.start ||
      !toRow?.start ||
      fromIndex === undefined ||
      toIndex === undefined
    ) {
      continue;
    }
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

// ─── 里程碑 ───────────────────────────────────────────────────────────

const milestoneMarks = computed(() =>
  milestones.map(milestone => ({
    ...milestone,
    left: dayIndex(milestone.date) * pxPerDay.value
  }))
);

// ─── 拖拽（移动 / 拉伸 / 未排期拖入） ─────────────────────────────────

const drag = ref<{
  code: string;
  kind: "move" | "resize";
  startX: number;
  origStart: string;
  origEffort: number;
} | null>(null);
const preview = ref<{ code: string; deltaDays: number } | null>(null);
const draggingUnscheduled = ref("");

function descendantsOf(code: string, seen = new Set<string>()): Set<string> {
  for (const dep of deps.filter(item => item.from === code)) {
    if (!seen.has(dep.to)) {
      seen.add(dep.to);
      descendantsOf(dep.to, seen);
    }
  }
  return seen;
}

function onBarPointerDown(
  row: Row,
  kind: "move" | "resize",
  event: PointerEvent
) {
  if (!row.start) return;
  drag.value = {
    code: row.code,
    kind,
    startX: event.clientX,
    origStart: row.start,
    origEffort: row.effortDays
  };
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
  preview.value = { code: drag.value.code, deltaDays };
}

function onBarPointerUp() {
  const dragInfo = drag.value;
  const move = preview.value;
  drag.value = null;
  preview.value = null;
  if (!dragInfo || !move) return;
  if (dragInfo.kind === "resize") {
    // 拉伸只改工作量估算（天数），不挪日期，直接生效
    applyResize(move.deltaDays);
    return;
  }
  const affected = [...descendantsOf(move.code)].filter(code => {
    const row = rows.value.find(item => item.code === code);
    return row?.start;
  });
  pendingMove.value = { code: move.code, deltaDays: move.deltaDays, affected };
  confirmOpen.value = true;
}

const pendingMove = ref<{
  code: string;
  deltaDays: number;
  affected: string[];
} | null>(null);
const confirmOpen = ref(false);

const pendingMoveTarget = computed(() =>
  pendingMove.value
    ? (rows.value.find(row => row.code === pendingMove.value?.code) ?? null)
    : null
);

function applyMove() {
  const move = pendingMove.value;
  confirmOpen.value = false;
  if (!move) return;
  const shift = [move.code, ...move.affected];
  rows.value = rows.value.map(row => {
    if (!shift.includes(row.code) || !row.start) return row;
    return { ...row, start: addDays(row.start, move.deltaDays) };
  });
  pendingMove.value = null;
}

function cancelMove() {
  confirmOpen.value = false;
  pendingMove.value = null;
}

function applyResize(deltaDays: number) {
  const dragInfo = drag.value;
  if (!dragInfo) return;
  rows.value = rows.value.map(row =>
    row.code === dragInfo.code
      ? { ...row, effortDays: Math.max(1, dragInfo.origEffort + deltaDays) }
      : row
  );
}

function onDropSchedule(row: Row, event: DragEvent) {
  const code =
    draggingUnscheduled.value ||
    event.dataTransfer?.getData("text/plain") ||
    "";
  if (code !== row.code) return;
  const target = event.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  const offset = event.clientX - rect.left;
  const day = Math.max(
    0,
    Math.min(
      TIMELINE_DAYS - row.effortDays,
      Math.round(offset / pxPerDay.value)
    )
  );
  rows.value = rows.value.map(item =>
    item.code === row.code
      ? { ...item, start: addDays(TIMELINE_START, day) }
      : item
  );
  draggingUnscheduled.value = "";
}

// ─── 负载分布（按周 × 负责人） ────────────────────────────────────────

const assignees = computed(() =>
  [...new Set(rows.value.map(row => row.assignee))].sort()
);

function effortInWeek(assignee: string, week: number): number {
  let days = 0;
  for (const row of rows.value) {
    if (!row.start || row.assignee !== assignee) continue;
    const startIdx = dayIndex(row.start);
    const overlapStart = Math.max(startIdx, week * 7);
    const overlapEnd = Math.min(startIdx + row.effortDays, (week + 1) * 7);
    days += Math.max(0, overlapEnd - overlapStart);
  }
  return days;
}

const maxWeekLoad = computed(() => {
  let max = 1;
  for (const assignee of assignees.value) {
    for (let week = 0; week < weekCount.value; week += 1) {
      max = Math.max(max, effortInWeek(assignee, week));
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
  for (const dep of deps) {
    const fromRow = rows.value.find(row => row.code === dep.from);
    const toRow = rows.value.find(row => row.code === dep.to);
    if (!fromRow?.start || !toRow?.start) continue;
    if (
      dayIndex(toRow.start) <
      dayIndex(fromRow.start) + fromRow.effortDays - 1
    ) {
      issues.push({
        key: `order-${dep.from}-${dep.to}`,
        text: t("s04.tasks.timeline.badOrder", { code: dep.to }),
        tone: "danger"
      });
    }
  }
  scheduleIssues.value = issues;
}

// ─── AI 排期建议 / 版本 ───────────────────────────────────────────────

function decideRec(recId: string, decision: "accepted" | "rejected") {
  recDecisions.value = { ...recDecisions.value, [recId]: decision };
  if (decision !== "accepted") return;
  const rec = recs.value.find(item => item.id === recId);
  if (rec?.move) {
    const affected = [...descendantsOf(rec.move.code)].filter(code => {
      const row = rows.value.find(item => item.code === code);
      return row?.start;
    });
    pendingMove.value = {
      code: rec.move.code,
      deltaDays: rec.move.deltaDays,
      affected
    };
    confirmOpen.value = true;
  }
}

const pendingRecs = computed(() =>
  recs.value.filter(rec => !recDecisions.value[rec.id])
);

interface PlanVersion {
  label: string;
  rows: Row[];
}

const versions = ref<PlanVersion[]>([]);
const versionCounter = ref(0);
const compareCode = ref("");
const notice = ref("");

const compareOptions = computed(() =>
  versions.value.map(version => ({
    value: version.label,
    label: version.label
  }))
);

const compareRows = computed(() => {
  if (!compareCode.value) return [];
  const found = versions.value.find(
    version => version.label === compareCode.value
  );
  return found ? found.rows : [];
});

function savePlanVersion() {
  versionCounter.value += 1;
  const label = `v3.${versionCounter.value}`;
  versions.value = [
    ...versions.value,
    { label, rows: rows.value.map(row => ({ ...row })) }
  ];
  notice.value = t("s04.tasks.timeline.versionSaved", { version: label });
}

function exitCompare() {
  compareCode.value = "";
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
        data-testid="tl-validate"
        @click="validateSchedule"
      >
        <CalendarCheck :size="14" />
        {{ t("s04.tasks.planning.validateSchedule") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        data-testid="tl-save"
        @click="savePlanVersion"
      >
        {{ t("s04.tasks.planning.savePlan") }}
      </Button>
    </template>

    <TasksFrame />
    <PlanningViewTabs />

    <Alert variant="info" :title="t('s04.tasks.planning.mockNote')" />
    <Alert
      v-if="notice"
      variant="success"
      :title="notice"
      data-testid="tl-notice"
    />

    <div class="tl-metrics">
      <span class="tl-metric"
        >{{ t("s04.tasks.planning.metricsVersion") }}: v3</span
      >
      <span class="tl-metric"
        >{{ t("s04.tasks.planning.metricsScheduled") }}:
        {{ scheduledRows.length }}</span
      >
      <span class="tl-metric"
        >{{ t("s04.tasks.planning.metricsUnscheduled") }}:
        {{ unscheduledRows.length }}</span
      >
      <span class="tl-metric"
        >{{ t("s04.tasks.planning.metricsMilestones") }}:
        {{ milestones.length }}</span
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
      <label class="tl-check">
        <input v-model="showMilestones" type="checkbox" />
        {{ t("s04.tasks.timeline.showMilestones") }}
      </label>
      <div class="tl-toolbar__spacer" />
      <Select
        v-if="versions.length > 0 && !compareCode"
        :options="compareOptions"
        :placeholder="t('s04.tasks.planning.compareVersions')"
        data-testid="tl-compare-select"
        @update:model-value="value => (compareCode = String(value))"
      />
      <template v-if="compareCode">
        <span class="tl-comparing">{{
          t("s04.tasks.timeline.comparing", { version: compareCode })
        }}</span>
        <Button
          variant="ghost"
          size="sm"
          data-testid="tl-exit-compare"
          @click="exitCompare"
        >
          {{ t("s04.tasks.timeline.exitCompare") }}
        </Button>
      </template>
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
            <div class="tl-header-track" :style="{ width: `${canvasWidth}px` }">
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
                  >2026-11 / 12</span
                >
              </div>
            </div>
          </div>

          <div v-if="showMilestones" class="tl-ms-row">
            <div class="tl-left-cell tl-left-cell--ms" />
            <div class="tl-ms-track" :style="{ width: `${canvasWidth}px` }">
              <span
                v-for="milestone in milestoneMarks"
                :key="milestone.id"
                class="tl-milestone"
                :style="{ left: `${milestone.left}px` }"
                :title="`${milestone.name} · ${t('s04.tasks.timeline.milestoneDue', { date: fmtDay(milestone.date) })}`"
              >
                ◆ {{ milestone.name }}
              </span>
            </div>
          </div>

          <div
            v-for="(item, index) in renderRows"
            :key="`${item.featureId}-${index}`"
            class="tl-row"
            :class="{ 'tl-row--feature': item.kind === 'feature' }"
          >
            <div
              class="tl-left-cell"
              :class="{ 'tl-left-cell--feature': item.kind === 'feature' }"
            >
              <button
                v-if="item.kind === 'feature'"
                type="button"
                class="tl-feature"
                @click="toggleFeature(item.featureId)"
              >
                {{ collapsed.has(item.featureId) ? "▸" : "▾" }}
                {{ item.featureName }}
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
                    'is-preview': preview?.code === item.row.code
                  }"
                  :style="{
                    left: `${barLeft(item.row)}px`,
                    width: `${barWidth(item.row)}px`
                  }"
                  :data-testid="`tl-bar-${item.row.code}`"
                  :data-start="item.row.start"
                  :data-effort="item.row.effortDays"
                  draggable="false"
                  @pointerdown="onBarPointerDown(item.row, 'move', $event)"
                  @pointermove="onBarPointerMove"
                  @pointerup="onBarPointerUp"
                >
                  <span class="tl-bar__label"
                    >{{ item.row.effortDays }}d · {{ item.row.assignee }}</span
                  >
                  <span
                    class="tl-bar__handle"
                    @pointerdown.stop="
                      onBarPointerDown(item.row, 'resize', $event)
                    "
                  />
                </div>
                <div
                  v-for="ghost in compareRows.filter(
                    candidate =>
                      candidate.code === item.row.code && candidate.start
                  )"
                  :key="`ghost-${ghost.code}`"
                  class="tl-bar tl-bar--ghost"
                  :style="{
                    left: `${barLeft(ghost)}px`,
                    width: `${barWidth(ghost)}px`
                  }"
                />
              </template>
              <span v-else class="tl-unscheduled-mark">{{
                t("s04.tasks.planning.metricsUnscheduled")
              }}</span>
            </div>
            <div
              v-else-if="item.kind === 'feature'"
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
            :key="row.code"
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
      <div class="tl-workload__grid">
        <div class="tl-workload__head">
          <span />
          <span v-for="label in weekLabels" :key="label">{{ label }}</span>
        </div>
        <div
          v-for="assignee in assignees"
          :key="assignee"
          class="tl-workload__row"
          :data-testid="`tl-load-${assignee}`"
        >
          <span class="tl-workload__name">{{ assignee }}</span>
          <span v-for="week in weekCount" :key="week" class="tl-workload__cell">
            <i
              class="tl-workload__bar"
              :style="{
                height: `${(effortInWeek(assignee, week - 1) / maxWeekLoad) * 100}%`
              }"
              :title="`${effortInWeek(assignee, week - 1)}d`"
            />
          </span>
        </div>
      </div>
    </section>

    <section class="tl-recs" data-testid="tl-recs">
      <h2>
        <Sparkles :size="14" />
        {{ t("s04.tasks.planning.aiTitle") }}
      </h2>
      <ul>
        <li
          v-for="rec in pendingRecs"
          :key="rec.id"
          class="tl-rec"
          :data-testid="`tl-rec-${rec.id}`"
        >
          <p>{{ rec.text }}</p>
          <p class="tl-rec__impact">{{ rec.impact }}</p>
          <div class="tl-rec__actions">
            <Button
              variant="secondary"
              size="sm"
              @click="decideRec(rec.id, 'accepted')"
            >
              {{ t("s04.tasks.planning.aiAccept") }}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              @click="decideRec(rec.id, 'rejected')"
            >
              {{ t("s04.tasks.planning.aiReject") }}
            </Button>
          </div>
        </li>
        <li
          v-for="decidedId in Object.keys(recDecisions)"
          :key="`done-${decidedId}`"
          class="tl-rec tl-rec--decided"
        >
          {{
            recDecisions[decidedId] === "accepted"
              ? t("s04.tasks.planning.aiAccepted")
              : t("s04.tasks.planning.aiRejected")
          }}
          · {{ decidedId }}
        </li>
      </ul>
    </section>

    <ConfirmationDialog
      :open="confirmOpen"
      :title="t('s04.tasks.timeline.confirmMoveTitle')"
      :description="
        pendingMove
          ? t('s04.tasks.timeline.confirmMoveDesc', {
              code: pendingMove.code,
              delta:
                pendingMove.deltaDays > 0
                  ? `+${pendingMove.deltaDays}`
                  : `${pendingMove.deltaDays}`,
              count: pendingMove.affected.length
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

.tl-comparing {
  color: var(--ws-color-info);
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

.tl-ms-row {
  display: flex;
  height: var(--space-4);
}

.tl-left-cell--ms {
  height: var(--space-4);
  min-height: var(--space-4);
  border-right: var(--border-1) solid var(--ws-border);
}

.tl-ms-track {
  position: relative;
}

.tl-milestone {
  position: absolute;
  top: 0;
  color: var(--ws-color-warning);
  font-size: var(--font-size-micro);
  white-space: nowrap;
  pointer-events: none;
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

.tl-bar--ghost {
  background: none;
  border: var(--border-1) dashed var(--ws-text-disabled);
  pointer-events: none;
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
.tl-workload,
.tl-recs {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-4) var(--space-5);
}

.tl-issues h2,
.tl-workload h2,
.tl-recs h2 {
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
  grid-template-columns: var(--space-16) repeat(6, minmax(0, 1fr));
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

.tl-recs ul {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.tl-rec {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3);
  border: var(--border-1) dashed var(--ws-color-info);
  border-radius: var(--radius-sm);
}

.tl-rec--decided {
  border: none;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.tl-rec p {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.tl-rec__impact {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.tl-rec__actions {
  display: flex;
  gap: var(--space-2);
}

@media (max-width: 1100px) {
  .tl-left-cell {
    width: calc(var(--space-24) * 2);
    min-width: calc(var(--space-24) * 2);
  }
}
</style>
