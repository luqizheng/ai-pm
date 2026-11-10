<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Minus, Plus, RefreshCw, Scan, Waypoints } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import { getDependencyGraph } from "@/api/task-plans";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import TaskStatusChip from "../components/TaskStatusChip.vue";
import { useDesignProject } from "../../design/use-design-project";
import PlanningViewTabs from "./PlanningViewTabs.vue";
import {
  toGraphView,
  validateGraph,
  type GraphIssue,
  type GraphViewModel
} from "./dependency-graph-adapter";

/**
 * 执行规划 · 依赖图页根（S04-UI-05，接真）。
 *
 * 数据 = pm-api REST（GET /v1/pm/projects/{project_id}/dependency-graph）：
 * 节点（标题/状态/优先级/排期/仓库）+ FS 依赖边 + **后端算好的**分层、
 * 关键路径、并行组与环检测。前端只做布局与高亮，不再自行推导拓扑。
 *
 * 诚实缺位：依赖关系**只读**（后端没有依赖增删写端点），因此检查器不提供
 * 增删依赖；「依赖原因 / 并行组前置条件与冲突说明 / AI 规划建议」后端无
 * 来源，渲染空态，不用演示文案填充。
 */
const router = useRouter();
const { t } = useI18n();
const { projectId, hasProject } = useDesignProject();

const graph = ref<GraphViewModel | null>(null);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!hasProject.value) {
    graph.value = null;
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    graph.value = toGraphView(await getDependencyGraph(projectId.value));
  } catch {
    loadError.value = true;
    graph.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const nodes = computed(() => graph.value?.nodes ?? []);
const edges = computed(() => graph.value?.edges ?? []);

// ─── 分层布局（层号来自后端 layers） ──────────────────────────────────

const NODE_W = 176;
const NODE_H = 72;
const GAP_X = 56;
const GAP_Y = 28;
const MARGIN = 24;

const layout = computed(() => {
  const positions = new Map<string, { x: number; y: number }>();
  const layers = graph.value?.layers ?? [];
  layers.forEach((layer, index) => {
    (layer ?? []).forEach((id, row) => {
      positions.set(id, {
        x: MARGIN + index * (NODE_W + GAP_X),
        y: MARGIN + row * (NODE_H + GAP_Y)
      });
    });
  });
  // 兜底：未出现在 layers 的节点追加到最后一列（不静默丢弃）
  const orphan = nodes.value.filter(node => !positions.has(node.id));
  orphan.forEach((node, index) => {
    positions.set(node.id, {
      x: MARGIN + layers.length * (NODE_W + GAP_X),
      y: MARGIN + index * (NODE_H + GAP_Y)
    });
  });

  let maxRows = 0;
  for (const layer of layers) maxRows = Math.max(maxRows, layer.length);
  maxRows = Math.max(maxRows, orphan.length);

  const width = MARGIN * 2 + (layers.length + 1) * (NODE_W + GAP_X) - GAP_X;
  const height = MARGIN * 2 + maxRows * (NODE_H + GAP_Y) - GAP_Y;
  return { positions, width, height };
});

const edgePaths = computed(() =>
  edges.value
    .filter(edge => layout.value.positions.has(edge.to))
    .map(edge => {
      const to = layout.value.positions.get(edge.to);
      const from = layout.value.positions.get(edge.from);
      if (!to) return null;
      const d = from
        ? `M ${from.x + NODE_W} ${from.y + NODE_H / 2} H ${(from.x + NODE_W + to.x) / 2} V ${to.y + NODE_H / 2} H ${to.x}`
        : `M 0 ${to.y + NODE_H / 2} H ${to.x}`;
      return {
        key: `${edge.from}->${edge.to}`,
        from: edge.from,
        to: edge.to,
        d,
        critical: edge.critical,
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

function nodeOf(id: string) {
  return nodes.value.find(node => node.id === id) ?? null;
}

function descendantsOf(code: string, seen = new Set<string>()): Set<string> {
  for (const edge of edges.value.filter(item => item.from === code)) {
    if (!seen.has(edge.to)) {
      seen.add(edge.to);
      descendantsOf(edge.to, seen);
    }
  }
  return seen;
}

function ancestorsOf(code: string, seen = new Set<string>()): Set<string> {
  for (const edge of edges.value.filter(item => item.to === code)) {
    if (!seen.has(edge.from)) {
      seen.add(edge.from);
      ancestorsOf(edge.from, seen);
    }
  }
  return seen;
}

const selected = computed(() => nodeOf(selectedCode.value));
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
  const node = nodeOf(code);
  const related =
    !selectedCode.value ||
    code === selectedCode.value ||
    ancestorsOf(selectedCode.value).has(code) ||
    descendantsOf(selectedCode.value).has(code);
  return {
    "is-selected": selectedCode.value === code,
    "is-dimmed": !related,
    "is-critical": Boolean(node?.critical),
    "is-parallel": Boolean(node?.parallel),
    "is-blocked": node?.status === "blocked"
  };
}

function edgeClass(from: string, to: string) {
  const related =
    !selectedCode.value ||
    from === selectedCode.value ||
    to === selectedCode.value ||
    ancestorsOf(selectedCode.value).has(from) ||
    descendantsOf(selectedCode.value).has(to);
  return {
    "is-dimmed": !related,
    "is-critical": Boolean(nodeOf(from)?.critical && nodeOf(to)?.critical)
  };
}

function openTask(id: string) {
  void router.push(`/project/tasks/${id}`);
}

// ─── 依赖校验（全部由真实读模型推导） ─────────────────────────────────

const validationIssues = ref<GraphIssue[] | null>(null);
const validatedAt = ref("");

function validate() {
  if (!graph.value) return;
  validationIssues.value = validateGraph(graph.value, {
    cycle: codes => t("s04.tasks.planning.issueCycle", { codes }),
    blockedUpstream: code =>
      t("s04.tasks.planning.issueBlockedUpstream", { code }),
    repoConflict: (a, b, repo) =>
      t("s04.tasks.planning.issueRepoConflict", { a, b, repo })
  });
  validatedAt.value = new Date().toLocaleTimeString();
}

// ─── 并行组（后端计算） ────────────────────────────────────────────────

const groups = computed(() => graph.value?.parallelGroups ?? []);
const groupedIds = computed(
  () => new Set(groups.value.flatMap(group => group.taskIds))
);
const ungroupedNodes = computed(() =>
  nodes.value.filter(node => !groupedIds.value.has(node.id))
);

const viewMode = ref<"graph" | "groups">("graph");
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
        data-testid="pg-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s04.tasks.common.refresh") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        data-testid="pg-validate"
        :disabled="!graph"
        @click="validate"
      >
        {{ t("s04.tasks.planning.validateDeps") }}
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
      v-else-if="!loading && nodes.length === 0"
      :title="t('s04.tasks.common.empty')"
      :description="t('s04.tasks.planning.noTasksHint')"
    />

    <template v-else>
      <Alert variant="info" :title="t('s04.tasks.planning.readOnlyNote')" />

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
              <svg
                class="pg-edges"
                :width="layout.width"
                :height="layout.height"
              >
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
                    <path
                      d="M0,0 L8,4 L0,8 Z"
                      class="pg-edge-arrow--critical"
                    />
                  </marker>
                </defs>
                <path
                  v-for="edge in edgePaths"
                  :key="edge.key"
                  :d="edge.d"
                  class="pg-edge"
                  :class="edgeClass(edge.from, edge.to)"
                  :marker-end="
                    edge.critical ? 'url(#pg-arrow-critical)' : 'url(#pg-arrow)'
                  "
                  fill="none"
                />
              </svg>
              <button
                v-for="task in nodes"
                :key="task.id"
                type="button"
                class="pg-node"
                :class="nodeClass(task.id)"
                :style="{
                  left: `${layout.positions.get(task.id)?.x}px`,
                  top: `${layout.positions.get(task.id)?.y}px`,
                  width: `${NODE_W}px`,
                  height: `${NODE_H}px`
                }"
                :data-testid="`pg-node-${task.code}`"
                :data-critical="task.critical ? 'true' : 'false'"
                :data-parallel="task.parallel ? 'true' : 'false'"
                :title="task.title"
                @click="selectedCode = task.id"
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
                    :data-testid="`pg-upstream-${nodeOf(edge.from)?.code}`"
                    @click="selectedCode = edge.from"
                  >
                    {{ nodeOf(edge.from)?.code }} ·
                    {{ nodeOf(edge.from)?.title }}
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
                    :data-testid="`pg-downstream-${nodeOf(edge.to)?.code}`"
                    @click="selectedCode = edge.to"
                  >
                    {{ nodeOf(edge.to)?.code }} · {{ nodeOf(edge.to)?.title }}
                  </button>
                </li>
              </ul>
            </section>
            <Button variant="ghost" size="sm" @click="openTask(selected.id)">
              {{ t("s04.tasks.planning.viewTask") }}
            </Button>
          </section>

          <section class="pg-panel" data-testid="pg-recs">
            <h2>{{ t("s04.tasks.planning.aiTitle") }}</h2>
            <EmptyState
              :title="t('s04.tasks.planning.aiUnavailable')"
              :description="t('s04.tasks.planning.aiUnavailableHint')"
              size="sm"
            />
          </section>

          <section
            v-if="validationIssues"
            class="pg-panel"
            data-testid="pg-validation"
          >
            <h2>{{ t("s04.tasks.planning.validationTitle") }}</h2>
            <p class="pg-panel__empty">
              {{
                t("s04.tasks.planning.validationRanAt", { time: validatedAt })
              }}
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
            <h3>
              {{ t("s04.tasks.planning.groupName", { index: group.index }) }}
            </h3>
            <span class="pg-group__count">{{ group.taskIds.length }}</span>
          </header>
          <dl class="pg-group__meta">
            <div>
              <dt>{{ t("s04.tasks.planning.groupRepos") }}</dt>
              <dd>
                <span v-for="repo in group.repos" :key="repo" class="bd-chip">{{
                  repo
                }}</span>
              </dd>
            </div>
          </dl>
          <ul class="pg-group__tasks">
            <li v-for="code in group.codes" :key="code">
              <span class="pg-node__code">{{ code }}</span>
              <span class="pg-group__task-title">{{
                nodeOf(group.taskIds[group.codes.indexOf(code)] ?? "")?.title
              }}</span>
            </li>
          </ul>
        </section>
        <section
          v-if="ungroupedNodes.length > 0"
          class="pg-group pg-group--unassigned"
          data-testid="pg-group-unassigned"
        >
          <header class="pg-group__head">
            <h3>{{ t("s04.tasks.common.unassigned") }}</h3>
          </header>
          <ul class="pg-group__tasks">
            <li v-for="node in ungroupedNodes" :key="node.id">
              <span class="pg-node__code">{{ node.code }}</span>
              <span class="pg-group__task-title">{{ node.title }}</span>
            </li>
          </ul>
        </section>
      </div>
    </template>
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
  text-align: left;
}

.pg-panel__empty {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
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

@media (max-width: 1100px) {
  .pg-workspace {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
