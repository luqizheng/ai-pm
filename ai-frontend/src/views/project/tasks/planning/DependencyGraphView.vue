<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Minus, Plus, Scan, Sparkles, Waypoints } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Input from "@/components/ui/input/Input.vue";
import Select from "@/components/ui/select/Select.vue";
import { PageFrame } from "@/components/shell";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import TaskStatusChip from "../components/TaskStatusChip.vue";
import PlanningViewTabs from "./PlanningViewTabs.vue";
import {
  planningEdges,
  planningGroupsFixture,
  planningRecsFixture,
  planningTasks,
  type ParallelGroupFixture,
  type PlanningRecFixture,
  type PlanningTaskFixture
} from "./fixtures";

/**
 * 执行规划 · 依赖图页根（S04-UI-05，mock 预览）。
 *
 * 项目级依赖图聚合端点未落地（后端仅有逐任务 upstream/downstream），
 * 本页以 fixtures 演示：分层布局的有向图（节点/依赖边/关键路径/阻塞/
 * 可并行 halo）、节点选择高亮上下游 + 依赖检查器（增删依赖含环路/
 * 重复/自依赖/未知编号校验）、并行执行组（第二视图）、AI 规划建议
 * （并行化采纳后落地为组）。全部改动仅存在于本页会话。
 */
const router = useRouter();
const { t } = useI18n();

type Task = PlanningTaskFixture;

const tasks: Task[] = planningTasks;
const taskByCode = new Map(tasks.map(task => [task.code, task]));
const edges = ref(
  planningEdges.filter(
    edge => taskByCode.has(edge.from) && taskByCode.has(edge.to)
  )
);
// 未知引用（T-060）保留在原始边里，供依赖校验发现问题
const rawEdges = ref([...planningEdges]);
const groups = ref<ParallelGroupFixture[]>(planningGroupsFixture());
const recs = ref<PlanningRecFixture[]>(planningRecsFixture());
const recDecisions = ref<Record<string, "accepted" | "rejected">>({});

const savedNotice = ref("");
const viewMode = ref<"graph" | "groups">("graph");

function taskOf(code: string): Task | undefined {
  return taskByCode.get(code);
}

// ─── 分层布局（最长路径分层；fixtures 保证无环） ─────────────────────

const NODE_W = 176;
const NODE_H = 72;
const GAP_X = 56;
const GAP_Y = 28;
const MARGIN = 24;

function predecessorsOf(code: string): string[] {
  return edges.value.filter(edge => edge.to === code).map(edge => edge.from);
}

function levelOf(code: string, seen: Set<string> = new Set()): number {
  if (seen.has(code)) return 0;
  seen.add(code);
  const preds = predecessorsOf(code);
  if (preds.length === 0) return 0;
  return Math.max(...preds.map(pred => levelOf(pred, seen))) + 1;
}

const layout = computed(() => {
  const levels = new Map<number, Task[]>();
  for (const task of tasks) {
    const level = levelOf(task.code);
    if (!levels.has(level)) levels.set(level, []);
    levels.get(level)?.push(task);
  }
  const positions = new Map<string, { x: number; y: number }>();
  let maxRows = 0;
  const sortedLevels = [...levels.keys()].sort((a, b) => a - b);
  sortedLevels.forEach(level => {
    const column = levels.get(level) ?? [];
    maxRows = Math.max(maxRows, column.length);
    column.forEach((task, index) => {
      positions.set(task.code, {
        x: MARGIN + level * (NODE_W + GAP_X),
        y: MARGIN + index * (NODE_H + GAP_Y)
      });
    });
  });
  const width = MARGIN * 2 + sortedLevels.length * (NODE_W + GAP_X) - GAP_X;
  const height = MARGIN * 2 + maxRows * (NODE_H + GAP_Y) - GAP_Y;
  return { positions, width, height };
});

const edgePaths = computed(() =>
  rawEdges.value
    .filter(edge => layout.value.positions.has(edge.to))
    .map(edge => {
      const to = layout.value.positions.get(edge.to);
      const from = layout.value.positions.get(edge.from);
      if (!to) return null;
      const critical = Boolean(
        taskOf(edge.from)?.critical && taskOf(edge.to)?.critical
      );
      // 未知引用（from 不存在）：从画布左缘引入，保留视觉痕迹供校验定位
      const d = from
        ? `M ${from.x + NODE_W} ${from.y + NODE_H / 2} H ${(from.x + NODE_W + to.x) / 2} V ${to.y + NODE_H / 2} H ${to.x}`
        : `M 0 ${to.y + NODE_H / 2} H ${to.x}`;
      return {
        key: `${edge.from}->${edge.to}`,
        from: edge.from,
        to: edge.to,
        d,
        critical,
        missing: !from
      };
    })
    .filter((item): item is NonNullable<typeof item> => item !== null)
);

// ─── 缩放 / 平移 ───────────────────────────────────────────────────────

const scale = ref(1);
const pan = ref({ x: 0, y: 0 });

function zoomIn() {
  scale.value = Math.min(1.6, Math.round((scale.value + 0.1) * 10) / 10);
}

function zoomOut() {
  scale.value = Math.max(0.4, Math.round((scale.value - 0.1) * 10) / 10);
}

function fit() {
  scale.value = 0.8;
  pan.value = { x: 0, y: 0 };
}

let panning = false;
let panStart = { x: 0, y: 0 };

function onPanStart(event: PointerEvent) {
  panning = true;
  panStart = { x: event.clientX - pan.value.x, y: event.clientY - pan.value.y };
}

function onPanMove(event: PointerEvent) {
  if (!panning) return;
  pan.value = {
    x: event.clientX - panStart.x,
    y: event.clientY - panStart.y
  };
}

function onPanEnd() {
  panning = false;
}

// ─── 选择 / 检查器 ─────────────────────────────────────────────────────

const selectedCode = ref("");

function descendantsOf(
  code: string,
  seen: Set<string> = new Set()
): Set<string> {
  for (const edge of edges.value.filter(item => item.from === code)) {
    if (!seen.has(edge.to)) {
      seen.add(edge.to);
      descendantsOf(edge.to, seen);
    }
  }
  return seen;
}

function ancestorsOf(code: string, seen: Set<string> = new Set()): Set<string> {
  for (const edge of edges.value.filter(item => item.to === code)) {
    if (!seen.has(edge.from)) {
      seen.add(edge.from);
      ancestorsOf(edge.from, seen);
    }
  }
  return seen;
}

const selected = computed(() => taskOf(selectedCode.value) ?? null);
const selectedUpstream = computed(() =>
  selectedCode.value
    ? edges.value.filter(edge => edge.to === selectedCode.value)
    : []
);
const selectedDownstream = computed(() =>
  selectedCode.value
    ? edges.value.filter(edge => edge.from === selectedCode.value)
    : []
);

function nodeClass(code: string) {
  const task = taskOf(code);
  const related =
    !selectedCode.value ||
    code === selectedCode.value ||
    ancestorsOf(selectedCode.value).has(code) ||
    descendantsOf(selectedCode.value).has(code);
  return {
    "is-selected": selectedCode.value === code,
    "is-dimmed": !related,
    "is-critical": Boolean(task?.critical),
    "is-parallel": Boolean(task?.parallel),
    "is-blocked": task?.status === "blocked"
  };
}

function edgeClass(key: string, from: string, to: string) {
  const related =
    !selectedCode.value ||
    from === selectedCode.value ||
    to === selectedCode.value ||
    ancestorsOf(selectedCode.value).has(from) ||
    descendantsOf(selectedCode.value).has(to);
  const critical = Boolean(taskOf(from)?.critical && taskOf(to)?.critical);
  return {
    "is-dimmed": !related,
    "is-critical": critical
  };
}

// ─── 依赖增删（自依赖 / 未知 / 重复 / 环路校验） ──────────────────────

const depInput = ref("");
const depError = ref("");

function wouldCycle(from: string, to: string): boolean {
  const stack = [to];
  const seen = new Set<string>();
  while (stack.length > 0) {
    const current = stack.pop() as string;
    if (current === from) return true;
    if (seen.has(current)) continue;
    seen.add(current);
    for (const edge of edges.value.filter(item => item.from === current)) {
      stack.push(edge.to);
    }
  }
  return false;
}

function submitDependency() {
  depError.value = "";
  if (!selectedCode.value) return;
  const target = depInput.value.trim();
  if (!target) return;
  if (target === selectedCode.value) {
    depError.value = t("s04.tasks.planning.errSelf");
    return;
  }
  if (!taskByCode.has(target)) {
    depError.value = t("s04.tasks.planning.errUnknown");
    return;
  }
  if (
    edges.value.some(
      edge => edge.from === target && edge.to === selectedCode.value
    )
  ) {
    depError.value = t("s04.tasks.planning.errDuplicate");
    return;
  }
  if (wouldCycle(target, selectedCode.value)) {
    depError.value = t("s04.tasks.planning.errCycle");
    return;
  }
  edges.value = [...edges.value, { from: target, to: selectedCode.value }];
  rawEdges.value = [
    ...rawEdges.value,
    { from: target, to: selectedCode.value }
  ];
  depInput.value = "";
}

function removeDependency(from: string) {
  if (!selectedCode.value) return;
  edges.value = edges.value.filter(
    edge => !(edge.from === from && edge.to === selectedCode.value)
  );
  rawEdges.value = rawEdges.value.filter(
    edge => !(edge.from === from && edge.to === selectedCode.value)
  );
}

function openTask(code: string) {
  void router.push(`/project/tasks/${code}`);
}

// ─── 依赖校验 ──────────────────────────────────────────────────────────

interface ValidationIssue {
  key: string;
  text: string;
  tone: "danger" | "warning";
}

const validationIssues = ref<ValidationIssue[] | null>(null);
const validatedAt = ref("");

function validate() {
  const issues: ValidationIssue[] = [];
  for (const edge of rawEdges.value) {
    if (!taskByCode.has(edge.from)) {
      issues.push({
        key: `ref-${edge.from}-${edge.to}`,
        text: t("s04.tasks.planning.issueInvalidRef", { code: edge.from }),
        tone: "danger"
      });
    }
    const source = taskOf(edge.from);
    if (source?.status === "blocked") {
      issues.push({
        key: `blocked-${edge.from}-${edge.to}`,
        text: t("s04.tasks.planning.issueBlockedUpstream", { code: edge.from }),
        tone: "warning"
      });
    }
  }
  const byModule = new Map<string, Task[]>();
  for (const task of tasks) {
    if (!byModule.has(task.module)) byModule.set(task.module, []);
    byModule.get(task.module)?.push(task);
  }
  for (const sameModule of byModule.values()) {
    if (sameModule.length < 2) continue;
    issues.push({
      key: `conflict-${sameModule[0].code}-${sameModule[1].code}`,
      text: t("s04.tasks.planning.issueConflict", {
        a: sameModule[0].code,
        b: sameModule[1].code
      }),
      tone: "warning"
    });
  }
  validationIssues.value = issues;
  validatedAt.value = new Date().toLocaleTimeString();
}

// ─── 并行组 ────────────────────────────────────────────────────────────

const groupOptions = computed(() =>
  groups.value.map(group => ({ value: group.id, label: group.name }))
);

function moveToGroup(taskCode: string, groupId: string) {
  groups.value = groups.value.map(group => {
    const without = group.taskCodes.filter(code => code !== taskCode);
    if (group.id === groupId) {
      return { ...group, taskCodes: [...without, taskCode] };
    }
    return { ...group, taskCodes: without };
  });
}

function unassignedTasks() {
  const grouped = new Set(groups.value.flatMap(group => group.taskCodes));
  return tasks.filter(task => !grouped.has(task.code));
}

// ─── AI 规划建议 ───────────────────────────────────────────────────────

function decideRec(id: string, decision: "accepted" | "rejected") {
  recDecisions.value = { ...recDecisions.value, [id]: decision };
  if (decision !== "accepted") return;
  const rec = recs.value.find(item => item.id === id);
  if (rec?.newGroup) {
    groups.value = [
      ...groups.value,
      {
        id: `G-AI-${id}`,
        name: rec.newGroup.name,
        taskCodes: [...rec.newGroup.taskCodes],
        preconditions: rec.newGroup.preconditions,
        repos: [...rec.newGroup.repos],
        conflicts: rec.newGroup.conflicts,
        rationale: rec.newGroup.rationale
      }
    ];
  }
}

const pendingRecs = computed(() =>
  recs.value.filter(rec => !recDecisions.value[rec.id])
);

const decidedRecs = computed(() =>
  recs.value
    .filter(rec => recDecisions.value[rec.id])
    .map(rec => ({
      id: rec.id,
      label:
        recDecisions.value[rec.id] === "accepted"
          ? t("s04.tasks.planning.aiAccepted")
          : t("s04.tasks.planning.aiRejected")
    }))
);

function savePlan() {
  savedNotice.value = t("s04.tasks.planning.savedNotice");
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
        data-testid="pg-validate"
        @click="validate"
      >
        {{ t("s04.tasks.planning.validateDeps") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        data-testid="pg-save"
        @click="savePlan"
      >
        {{ t("s04.tasks.planning.savePlan") }}
      </Button>
    </template>

    <TasksFrame />
    <PlanningViewTabs />

    <Alert variant="info" :title="t('s04.tasks.planning.mockNote')" />
    <Alert
      v-if="savedNotice"
      variant="success"
      :title="savedNotice"
      data-testid="pg-saved"
    />

    <div class="pg-toolbar">
      <div class="pg-toolbar__modes" role="tablist">
        <button
          type="button"
          class="pg-mode"
          :class="{ 'is-active': viewMode === 'graph' }"
          data-testid="pg-mode-graph"
          @click="viewMode = 'graph'"
        >
          <Waypoints :size="13" />
          {{ t("s04.tasks.planning.viewGraph") }}
        </button>
        <button
          type="button"
          class="pg-mode"
          :class="{ 'is-active': viewMode === 'groups' }"
          data-testid="pg-mode-groups"
          @click="viewMode = 'groups'"
        >
          {{ t("s04.tasks.planning.viewGroups") }}
        </button>
      </div>
      <div class="pg-toolbar__spacer" />
      <template v-if="viewMode === 'graph'">
        <Button
          variant="ghost"
          size="sm"
          :aria-label="t('s04.tasks.planning.zoomOut')"
          @click="zoomOut"
        >
          <Minus :size="13" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          :aria-label="t('s04.tasks.planning.zoomIn')"
          @click="zoomIn"
        >
          <Plus :size="13" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          :aria-label="t('s04.tasks.planning.fit')"
          @click="fit"
        >
          <Scan :size="13" />
        </Button>
      </template>
    </div>

    <div v-if="viewMode === 'graph'" class="pg-workspace">
      <section class="pg-canvas-col">
        <div class="pg-legend">
          <span class="pg-legend__item"
            ><i class="pg-dot pg-dot--danger" />{{
              t("s04.tasks.planning.legendBlocked")
            }}</span
          >
          <span class="pg-legend__item"
            ><i class="pg-dot pg-dot--critical" />{{
              t("s04.tasks.planning.legendCritical")
            }}</span
          >
          <span class="pg-legend__item"
            ><i class="pg-dot pg-dot--parallel" />{{
              t("s04.tasks.planning.legendParallel")
            }}</span
          >
          <span class="pg-legend__item"
            ><i class="pg-dot" />{{
              t("s04.tasks.planning.legendNormal")
            }}</span
          >
        </div>
        <div
          class="pg-canvas"
          data-testid="pg-canvas"
          @pointerdown="onPanStart"
          @pointermove="onPanMove"
          @pointerup="onPanEnd"
          @pointerleave="onPanEnd"
        >
          <div
            class="pg-canvas__layer"
            :style="{
              width: `${layout.width}px`,
              height: `${layout.height}px`,
              transform: `translate(${pan.x}px, ${pan.y}px) scale(${scale})`
            }"
          >
            <svg class="pg-edges" :width="layout.width" :height="layout.height">
              <defs>
                <marker
                  id="pg-arrow"
                  markerWidth="8"
                  markerHeight="8"
                  refX="7"
                  refY="4"
                  orient="auto"
                >
                  <path d="M0,0 L8,4 L0,8 Z" class="pg-edge-arrow" />
                </marker>
                <marker
                  id="pg-arrow-critical"
                  markerWidth="8"
                  markerHeight="8"
                  refX="7"
                  refY="4"
                  orient="auto"
                >
                  <path d="M0,0 L8,4 L0,8 Z" class="pg-edge-arrow--critical" />
                </marker>
              </defs>
              <path
                v-for="edge in edgePaths"
                :key="edge.key"
                :d="edge.d"
                class="pg-edge"
                :class="edgeClass(edge.key, edge.from, edge.to)"
                :marker-end="
                  edge.critical ? 'url(#pg-arrow-critical)' : 'url(#pg-arrow)'
                "
                fill="none"
              />
            </svg>
            <button
              v-for="task in tasks"
              :key="task.code"
              type="button"
              class="pg-node"
              :class="nodeClass(task.code)"
              :style="{
                left: `${layout.positions.get(task.code)?.x}px`,
                top: `${layout.positions.get(task.code)?.y}px`,
                width: `${NODE_W}px`,
                height: `${NODE_H}px`
              }"
              :data-testid="`pg-node-${task.code}`"
              :title="task.title"
              @click="selectedCode = task.code"
            >
              <span class="pg-node__code">
                {{ task.code }}
                <i v-if="task.critical" class="pg-node__flag">⚑</i>
              </span>
              <span class="pg-node__title">{{ task.title }}</span>
              <span class="pg-node__meta">
                <TaskPriorityChip :priority="task.priority" />
                <span class="pg-node__repo">{{ task.repo }}</span>
              </span>
            </button>
          </div>
        </div>
      </section>

      <aside class="pg-side">
        <section v-if="selected" class="pg-panel" data-testid="pg-inspector">
          <h2>{{ t("s04.tasks.planning.inspectorTitle") }}</h2>
          <p class="pg-panel__task">
            <span class="pg-node__code">{{ selected.code }}</span>
            {{ selected.title }}
            <TaskStatusChip :status="selected.status" />
          </p>
          <section class="pg-panel__section">
            <h3>{{ t("s04.tasks.planning.depReason") }}</h3>
            <p>{{ selected.reason }}</p>
          </section>
          <section class="pg-panel__section">
            <h3>{{ t("s04.tasks.planning.upstream") }}</h3>
            <p v-if="selectedUpstream.length === 0" class="pg-panel__empty">
              {{ t("s04.tasks.backlog.noDependencies") }}
            </p>
            <ul>
              <li
                v-for="edge in selectedUpstream"
                :key="`${edge.from}->${edge.to}`"
              >
                <button
                  type="button"
                  class="pg-panel__link"
                  @click="selectedCode = edge.from"
                >
                  {{ edge.from }}
                </button>
                <button
                  type="button"
                  class="pg-panel__remove"
                  :data-testid="`pg-dep-remove-${edge.from}`"
                  @click="removeDependency(edge.from)"
                >
                  {{ t("s04.tasks.planning.removeDep") }}
                </button>
              </li>
            </ul>
          </section>
          <section class="pg-panel__section">
            <h3>{{ t("s04.tasks.planning.downstream") }}</h3>
            <p v-if="selectedDownstream.length === 0" class="pg-panel__empty">
              {{ t("s04.tasks.backlog.noDependencies") }}
            </p>
            <ul>
              <li
                v-for="edge in selectedDownstream"
                :key="`${edge.from}->${edge.to}`"
              >
                <button
                  type="button"
                  class="pg-panel__link"
                  @click="selectedCode = edge.to"
                >
                  {{ edge.to }}
                </button>
              </li>
            </ul>
          </section>
          <section class="pg-panel__section">
            <h3>{{ t("s04.tasks.planning.addDep") }}</h3>
            <div class="pg-panel__add">
              <Input
                v-model="depInput"
                :placeholder="t('s04.tasks.planning.addDepPlaceholder')"
                data-testid="pg-dep-input"
              />
              <Button
                variant="secondary"
                size="sm"
                data-testid="pg-dep-add"
                @click="submitDependency"
              >
                {{ t("s04.tasks.planning.addDepBtn") }}
              </Button>
            </div>
            <p
              v-if="depError"
              class="pg-panel__error"
              data-testid="pg-dep-error"
            >
              {{ depError }}
            </p>
          </section>
          <Button variant="ghost" size="sm" @click="openTask(selected.code)">
            {{ t("s04.tasks.planning.viewTask") }}
          </Button>
        </section>

        <section class="pg-panel" data-testid="pg-recs">
          <h2>
            <Sparkles :size="14" />
            {{ t("s04.tasks.planning.aiTitle") }}
          </h2>
          <ul class="pg-recs">
            <li
              v-for="rec in pendingRecs"
              :key="rec.id"
              class="pg-rec"
              :data-testid="`pg-rec-${rec.id}`"
            >
              <p class="pg-rec__text">{{ rec.text }}</p>
              <p class="pg-rec__impact">
                {{ t("s04.tasks.planning.aiImpact") }}：{{ rec.impact }}
              </p>
              <div class="pg-rec__actions">
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
              v-for="decided in decidedRecs"
              :key="`done-${decided.id}`"
              class="pg-rec pg-rec--decided"
            >
              {{ decided.label }} · {{ decided.id }}
            </li>
          </ul>
        </section>

        <section
          v-if="validationIssues"
          class="pg-panel"
          data-testid="pg-validation"
        >
          <h2>{{ t("s04.tasks.planning.validationTitle") }}</h2>
          <p class="pg-panel__empty">
            {{ t("s04.tasks.planning.validationRanAt", { time: validatedAt }) }}
          </p>
          <EmptyState
            v-if="validationIssues.length === 0"
            :title="t('s04.tasks.planning.validationClean')"
            size="sm"
          />
          <ul v-else class="pg-issues">
            <li
              v-for="issue in validationIssues"
              :key="issue.key"
              class="pg-issue"
              :class="`pg-issue--${issue.tone}`"
            >
              {{ issue.text }}
            </li>
          </ul>
        </section>
      </aside>
    </div>

    <div v-else class="pg-groups" data-testid="pg-groups">
      <EmptyState
        v-if="groups.length === 0"
        :title="t('s04.tasks.planning.groupNone')"
        size="sm"
      />
      <section
        v-for="group in groups"
        :key="group.id"
        class="pg-group"
        :data-testid="`pg-group-${group.id}`"
      >
        <header class="pg-group__head">
          <h3>{{ group.name }}</h3>
          <span class="pg-group__count">{{ group.taskCodes.length }}</span>
        </header>
        <dl class="pg-group__meta">
          <div>
            <dt>{{ t("s04.tasks.planning.groupPreconditions") }}</dt>
            <dd>{{ group.preconditions }}</dd>
          </div>
          <div>
            <dt>{{ t("s04.tasks.planning.groupRepos") }}</dt>
            <dd>
              <span v-for="repo in group.repos" :key="repo" class="bd-chip">{{
                repo
              }}</span>
            </dd>
          </div>
          <div>
            <dt>{{ t("s04.tasks.planning.groupConflicts") }}</dt>
            <dd>{{ group.conflicts }}</dd>
          </div>
          <div>
            <dt>{{ t("s04.tasks.planning.groupRationale") }}</dt>
            <dd>{{ group.rationale }}</dd>
          </div>
        </dl>
        <ul class="pg-group__tasks">
          <li v-for="code in group.taskCodes" :key="code">
            <span class="pg-node__code">{{ code }}</span>
            <span class="pg-group__task-title">{{ taskOf(code)?.title }}</span>
            <label class="pg-group__move">
              <span class="pg-visually-hidden">{{
                t("s04.tasks.planning.groupMove")
              }}</span>
              <Select
                :model-value="group.id"
                :options="groupOptions"
                :aria-label="t('s04.tasks.planning.groupMove')"
                :data-testid="`pg-move-${code}`"
                @update:model-value="value => moveToGroup(code, String(value))"
              />
            </label>
          </li>
        </ul>
      </section>
      <section
        v-if="unassignedTasks().length > 0"
        class="pg-group pg-group--unassigned"
      >
        <header class="pg-group__head">
          <h3>{{ t("s04.tasks.common.unassigned") }}</h3>
        </header>
        <ul class="pg-group__tasks">
          <li v-for="task in unassignedTasks()" :key="task.code">
            <span class="pg-node__code">{{ task.code }}</span>
            <span class="pg-group__task-title">{{ task.title }}</span>
            <label class="pg-group__move">
              <Select
                :options="groupOptions"
                :placeholder="t('s04.tasks.planning.groupMove')"
                :data-testid="`pg-move-${task.code}`"
                @update:model-value="
                  value => value && moveToGroup(task.code, String(value))
                "
              />
            </label>
          </li>
        </ul>
      </section>
    </div>
  </PageFrame>
</template>

<style scoped>
.pg-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pg-toolbar__spacer {
  flex: 1;
}

.pg-toolbar__modes {
  display: inline-flex;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.pg-mode {
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

.pg-mode.is-active {
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
}

.pg-workspace {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(var(--space-12), 28%);
  gap: var(--space-5);
  align-items: start;
}

.pg-canvas-col {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.pg-legend {
  display: flex;
  gap: var(--space-4);
}

.pg-legend__item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.pg-dot {
  width: var(--space-2);
  height: var(--space-2);
  border-radius: var(--radius-full);
  background: var(--ws-text-disabled);
  display: inline-block;
}

.pg-dot--danger {
  background: var(--ws-color-danger);
}

.pg-dot--critical {
  background: var(--ws-accent);
}

.pg-dot--parallel {
  background: none;
  border: var(--border-1) dashed var(--ws-color-info);
}

.pg-canvas {
  position: relative;
  overflow: hidden;
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  height: calc(var(--space-24) * 5);
  cursor: grab;
  touch-action: none;
}

.pg-canvas__layer {
  position: absolute;
  top: 0;
  left: 0;
  transform-origin: top left;
}

.pg-edges {
  position: absolute;
  top: 0;
  left: 0;
}

.pg-edge {
  stroke: var(--ws-text-disabled);
  stroke-width: var(--border-1);
}

.pg-edge.is-critical {
  stroke: var(--ws-accent);
  stroke-width: 2px;
}

.pg-edge.is-dimmed {
  opacity: 0.25;
}

.pg-edge-arrow {
  fill: var(--ws-text-disabled);
}

.pg-edge-arrow--critical {
  fill: var(--ws-accent);
}

.pg-node {
  position: absolute;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  text-align: left;
  overflow: hidden;
}

.pg-node:hover {
  border-color: var(--ws-accent);
}

.pg-node:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.pg-node.is-selected {
  border-color: var(--ws-accent);
  box-shadow: none;
  outline: var(--size-focus-ring) solid var(--ws-accent);
}

.pg-node.is-dimmed {
  opacity: 0.3;
}

.pg-node.is-critical {
  border-color: var(--ws-accent);
}

.pg-node.is-parallel {
  border-style: dashed;
  border-color: var(--ws-color-info);
}

.pg-node.is-blocked {
  border-color: var(--ws-color-danger);
}

.pg-node__code {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.pg-node__flag {
  color: var(--ws-accent);
  font-style: normal;
}

.pg-node__title {
  width: 100%;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pg-node__meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pg-node__repo {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.pg-side {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.pg-panel {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-4) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.pg-panel h2 {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.pg-panel__task {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.pg-panel__section h3 {
  margin: 0 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.pg-panel__section p {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.pg-panel__section ul {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.pg-panel__section li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.pg-panel__link {
  border: none;
  background: none;
  padding: 0;
  color: var(--ws-accent);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.pg-panel__remove {
  border: none;
  background: none;
  padding: 0;
  color: var(--ws-color-danger);
  font-size: var(--font-size-micro);
  cursor: pointer;
}

.pg-panel__empty {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.pg-panel__add {
  display: flex;
  gap: var(--space-2);
}

.pg-panel__add > :first-child {
  flex: 1;
}

.pg-panel__error {
  margin: 0;
  color: var(--ws-color-danger);
  font-size: var(--font-size-caption);
}

.pg-recs {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.pg-rec {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3);
  border: var(--border-1) dashed var(--ws-color-info);
  border-radius: var(--radius-sm);
}

.pg-rec--decided {
  border: none;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.pg-rec__text {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.pg-rec__impact {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.pg-rec__actions {
  display: flex;
  gap: var(--space-2);
}

.pg-issues {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.pg-issue {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
}

.pg-issue--danger {
  border: var(--border-1) solid var(--ws-color-danger);
  color: var(--ws-color-danger);
}

.pg-issue--warning {
  border: var(--border-1) solid var(--ws-color-warning);
  color: var(--ws-color-warning);
}

.pg-groups {
  display: grid;
  grid-template-columns: repeat(
    auto-fill,
    minmax(min(100%, calc(var(--space-24) * 3)), 1fr)
  );
  gap: var(--space-4);
}

.pg-group {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-4) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.pg-group--unassigned {
  border-style: dashed;
}

.pg-group__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.pg-group__head h3 {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.pg-group__count {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.pg-group__meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.pg-group__meta dt {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.pg-group__meta dd {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.bd-chip {
  padding: 0 var(--space-2);
  height: var(--size-control-sm);
  display: inline-flex;
  align-items: center;
  border-radius: var(--radius-full);
  background: var(--ws-elevated);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.pg-group__tasks {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.pg-group__tasks li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.pg-group__task-title {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.pg-group__move {
  min-width: calc(var(--space-24) + var(--space-16));
}

.pg-visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}

@media (max-width: 1100px) {
  .pg-workspace {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
