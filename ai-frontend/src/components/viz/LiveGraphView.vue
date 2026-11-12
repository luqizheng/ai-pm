<script setup lang="ts">
/**
 * LiveGraphView —— Live 在线交互图（P1）。
 *
 * 四项交互（十号任务书 §2，Adapter A）：
 * - hover trace：悬停节点高亮入边/出边，其余降透明（G6 hover-activate）；
 * - focus panel：点击节点 → emit selectNode（父级接既有 VizTracePanel）；
 * - route journey：选起点终点 → 前端 BFS（只走已投影边）→ 高亮 + 步骤列表；
 * - lens：按节点 kind / 边 relation 过滤高亮。
 *
 * 纪律：
 * - **lazy 加载**：G6 adapter（连带 @antv/g6）在本组件内动态 import，
 *   不得从主入口静态依赖（V-1 零主 bundle 污染）；
 * - **降级不阻断**：加载或渲染失败 → 降级 VizSpecTable（表格），页面不报错；
 * - 权限与一致性：数据来自 Live 端点（服务端已裁剪），前端不请求"全量"；
 *   `meta.consistency = mixed-revision` 时必须提示（不静默展示成同一时点）；
 * - 交互只改变视图，零写路径（ADR-VIZ-005）。
 */
import {
  computed,
  nextTick,
  onBeforeUnmount,
  ref,
  shallowRef,
  watch
} from "vue";
import { useI18n } from "vue-i18n";
import { getLiveGraph, getLiveSpec } from "@/api/visualizations";
import type { VizLiveResponse, VizNode, VizSpec } from "@/lib/viz/types";
import { toLiveSpec } from "@/lib/viz/specAdapter";
import VizSpecTable from "./VizSpecTable.vue";

const props = defineProps<{
  projectId: string;
  viewKind: string;
}>();

const emit = defineEmits<{
  selectNode: [node: VizNode, spec: VizSpec];
}>();

const { t } = useI18n();

const spec = ref<VizSpec | null>(null);
const consistency = ref<VizLiveResponse["meta"]["consistency"]>("current");
const truncated = ref(false);
const loading = ref(false);
/** 加载失败（端点/契约）——降级表格。 */
const loadFailed = ref(false);
/** 渲染失败（图库）——降级表格，页面不报错。 */
const degraded = ref(false);
const mode = ref<"graph" | "table">("graph");

const containerRef = ref<HTMLElement | null>(null);
/** 渲染器实例（持 G6 大对象，shallowRef 避免深响应开销）。 */
const renderer = shallowRef<
  import("@/lib/viz/renderers/capabilities").VizGraphRenderer | null
>(null);

const journeyFrom = ref("");
const journeyTo = ref("");
const journeyPath = ref<string[] | null>(null);
/** 已执行过查询且不可达（区别于"尚未查询"）。 */
const journeyMissed = ref(false);

const lensKind = ref("");
const lensRelation = ref("");

/** lens 下拉数据源（spec 就绪后计算）。 */
const lensOptions = computed(() => {
  if (!spec.value) return { kinds: [], relations: [] };
  const kinds = Array.from(new Set(spec.value.nodes.map(n => n.kind))).sort();
  const relations = Array.from(
    new Set(spec.value.edges.map(e => e.relation))
  ).sort();
  return { kinds, relations };
});

/** journey 起终点候选（spec 节点）。 */
const journeyCandidates = computed(() =>
  spec.value ? spec.value.nodes.map(n => ({ id: n.id, label: n.label })) : []
);

/** 路径步骤（节点 label 序列）。 */
const journeySteps = computed(() => {
  if (!spec.value || !journeyPath.value) return [];
  const byId = new Map(spec.value.nodes.map(n => [n.id, n.label]));
  return journeyPath.value.map(id => byId.get(id) ?? id);
});

/** view_kind → 标题文案 key。 */
const titleKey = computed(() => {
  switch (props.viewKind) {
    case "trace-graph":
      return "viz.live.viewTrace";
    case "task-dag":
      return "viz.live.viewDag";
    case "architecture-graph":
      return "viz.live.viewArchitecture";
    default:
      return "viz.live.title";
  }
});

async function destroyRenderer(): Promise<void> {
  const current = renderer.value;
  renderer.value = null;
  journeyPath.value = null;
  if (current) {
    try {
      current.destroy();
    } catch {
      // 释放失败不阻断页面（卸载路径）
    }
  }
}

async function load(): Promise<void> {
  if (!props.projectId) {
    spec.value = null;
    return;
  }
  loading.value = true;
  loadFailed.value = false;
  degraded.value = false;
  journeyFrom.value = "";
  journeyTo.value = "";
  journeyPath.value = null;
  journeyMissed.value = false;
  lensKind.value = "";
  lensRelation.value = "";
  mode.value = "graph";
  await destroyRenderer();
  try {
    const response = await getLiveGraph(props.projectId, props.viewKind);
    consistency.value = response.meta?.consistency ?? "current";
    truncated.value = response.meta?.truncated ?? false;
    spec.value = toLiveSpec(response);
    await mountGraph();
  } catch {
    loadFailed.value = true;
    spec.value = null;
  } finally {
    loading.value = false;
  }
}

/** 动态 import adapter（G6 只进 lazy chunk），失败即降级表格。 */
async function mountGraph(): Promise<void> {
  if (!spec.value) return;
  await nextTick();
  if (!containerRef.value) return;
  try {
    const { createG6Renderer } = await import("@/lib/viz/renderers/g6Adapter");
    renderer.value = await createG6Renderer({
      container: containerRef.value,
      spec: spec.value,
      onNodeClick: nodeId => {
        const node = spec.value?.nodes.find(n => n.id === nodeId);
        if (node && spec.value) emit("selectNode", node, spec.value);
      }
    });
  } catch {
    degraded.value = true;
    mode.value = "table";
  }
}

function selectNode(node: VizNode): void {
  if (spec.value) emit("selectNode", node, spec.value);
}

/** lens 应用：两维全空 = 清除。 */
async function onLensChange(): Promise<void> {
  const current = renderer.value;
  if (!current || !current.capabilities.lens) return;
  journeyPath.value = null;
  const selection = {
    ...(lensKind.value ? { kind: lensKind.value } : {}),
    ...(lensRelation.value ? { relation: lensRelation.value } : {})
  };
  try {
    if (selection.kind || selection.relation) {
      await current.applyLens(selection);
    } else {
      await current.clearLens();
    }
  } catch {
    // 状态操作失败不阻断页面
  }
}

/** route journey：前端 BFS（只走已投影边），命中即高亮。 */
async function onJourneyRun(): Promise<void> {
  const current = renderer.value;
  if (!current || !current.capabilities.routeJourney || !spec.value) return;
  if (!journeyFrom.value || !journeyTo.value) return;
  const path = current.findPath(journeyFrom.value, journeyTo.value);
  journeyPath.value = path;
  journeyMissed.value = path === null;
  try {
    if (path) {
      await current.highlightPath(path);
    } else {
      await current.clearPath();
    }
  } catch {
    // 高亮失败不阻断
  }
}

async function onJourneyClear(): Promise<void> {
  journeyFrom.value = "";
  journeyTo.value = "";
  journeyPath.value = null;
  journeyMissed.value = false;
  const current = renderer.value;
  try {
    await current?.clearPath();
  } catch {
    // 清除失败不阻断
  }
}

watch(
  () => [props.projectId, props.viewKind],
  () => {
    void load();
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  void destroyRenderer();
});
</script>

<template>
  <div class="live-graph" data-testid="live-graph">
    <header class="live-graph__toolbar">
      <h3 class="live-graph__title">{{ t(titleKey) }}</h3>
      <div
        v-if="!loadFailed && !degraded"
        class="live-graph__mode"
        role="group"
        :aria-label="t('viz.live.modeLabel')"
      >
        <button
          type="button"
          class="live-graph__mode-btn"
          :class="{ 'live-graph__mode-btn--on': mode === 'graph' }"
          data-testid="live-graph-mode-graph"
          @click="mode = 'graph'"
        >
          {{ t("viz.live.tabGraph") }}
        </button>
        <button
          type="button"
          class="live-graph__mode-btn"
          :class="{ 'live-graph__mode-btn--on': mode === 'table' }"
          data-testid="live-graph-mode-table"
          @click="mode = 'table'"
        >
          {{ t("viz.live.tabTable") }}
        </button>
      </div>
    </header>

    <p
      v-if="consistency !== 'current'"
      class="live-graph__notice"
      data-testid="live-graph-mixed"
    >
      {{ t("viz.live.mixedRevision") }}
    </p>
    <p
      v-if="truncated"
      class="live-graph__notice"
      data-testid="live-graph-truncated"
    >
      {{ t("viz.live.truncated") }}
    </p>
    <p
      v-if="loadFailed"
      class="live-graph__notice live-graph__notice--warn"
      data-testid="live-graph-load-failed"
    >
      {{ t("viz.live.loadFailed") }}
    </p>
    <p
      v-else-if="degraded"
      class="live-graph__notice live-graph__notice--warn"
      data-testid="live-graph-degraded"
    >
      {{ t("viz.live.renderDegraded") }}
    </p>

    <template v-if="spec && !loadFailed">
      <div
        v-if="!degraded"
        class="live-graph__controls"
        data-testid="live-graph-controls"
      >
        <label class="live-graph__field">
          <span class="live-graph__field-label">
            {{ t("viz.live.lensLabel") }}
          </span>
          <select
            v-model="lensKind"
            class="live-graph__select"
            data-testid="live-graph-lens-kind"
            :aria-label="t('viz.live.lensKind')"
            @change="onLensChange"
          >
            <option value="">{{ t("viz.live.lensAll") }}</option>
            <option v-for="kind in lensOptions.kinds" :key="kind" :value="kind">
              {{ kind }}
            </option>
          </select>
        </label>
        <select
          v-model="lensRelation"
          class="live-graph__select"
          data-testid="live-graph-lens-relation"
          :aria-label="t('viz.live.lensRelation')"
          @change="onLensChange"
        >
          <option value="">{{ t("viz.live.lensAll") }}</option>
          <option
            v-for="relation in lensOptions.relations"
            :key="relation"
            :value="relation"
          >
            {{ relation }}
          </option>
        </select>

        <span class="live-graph__divider" aria-hidden="true" />

        <label class="live-graph__field">
          <span class="live-graph__field-label">
            {{ t("viz.live.journeyFrom") }}
          </span>
          <select
            v-model="journeyFrom"
            class="live-graph__select"
            data-testid="live-graph-journey-from"
            :aria-label="t('viz.live.journeyFrom')"
          >
            <option value="" />
            <option
              v-for="candidate in journeyCandidates"
              :key="candidate.id"
              :value="candidate.id"
            >
              {{ candidate.label }}
            </option>
          </select>
        </label>
        <label class="live-graph__field">
          <span class="live-graph__field-label">
            {{ t("viz.live.journeyTo") }}
          </span>
          <select
            v-model="journeyTo"
            class="live-graph__select"
            data-testid="live-graph-journey-to"
            :aria-label="t('viz.live.journeyTo')"
          >
            <option value="" />
            <option
              v-for="candidate in journeyCandidates"
              :key="candidate.id"
              :value="candidate.id"
            >
              {{ candidate.label }}
            </option>
          </select>
        </label>
        <button
          type="button"
          class="live-graph__btn"
          data-testid="live-graph-journey-run"
          :disabled="!journeyFrom || !journeyTo"
          @click="onJourneyRun"
        >
          {{ t("viz.live.journeyRun") }}
        </button>
        <button
          v-if="journeyPath"
          type="button"
          class="live-graph__btn"
          data-testid="live-graph-journey-clear"
          @click="onJourneyClear"
        >
          {{ t("viz.live.journeyClear") }}
        </button>
      </div>

      <p
        v-if="spec.nodes.length === 0"
        class="live-graph__empty"
        data-testid="live-graph-empty"
      >
        {{ t("viz.live.empty") }}
      </p>

      <div
        v-show="mode === 'graph' && !degraded && spec.nodes.length > 0"
        ref="containerRef"
        class="live-graph__canvas"
        data-testid="live-graph-canvas"
      />

      <ol
        v-if="journeyPath"
        class="live-graph__journey"
        data-testid="live-graph-journey-path"
      >
        <li class="live-graph__journey-summary">
          {{ t("viz.live.journeySteps", { count: journeySteps.length }) }}
        </li>
        <li v-for="(step, index) in journeySteps" :key="`${step}-${index}`">
          {{ index + 1 }}. {{ step }}
        </li>
      </ol>
      <p
        v-else-if="journeyFrom && journeyTo && journeyMissed"
        class="live-graph__empty"
        data-testid="live-graph-journey-empty"
      >
        {{ t("viz.live.journeyEmpty") }}
      </p>

      <VizSpecTable
        v-if="mode === 'table' || degraded"
        :spec="spec"
        @select-node="selectNode"
      />
    </template>
  </div>
</template>

<style scoped>
.live-graph {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.live-graph__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.live-graph__title {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-primary);
}

.live-graph__mode {
  display: inline-flex;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  overflow: hidden;
}

.live-graph__mode-btn {
  padding: var(--space-1) var(--space-3);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  background: var(--surface-base);
  border: none;
  cursor: pointer;
}

.live-graph__mode-btn--on {
  color: var(--accent-contrast-text);
  background: var(--accent);
}

.live-graph__notice {
  margin: 0;
  padding: var(--space-2);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
}

.live-graph__notice--warn {
  color: var(--text-primary);
}

.live-graph__controls {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.live-graph__field {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}

.live-graph__field-label {
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.live-graph__select {
  max-width: 220px;
  padding: var(--space-1);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
}

.live-graph__btn {
  padding: var(--space-1) var(--space-3);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.live-graph__btn:disabled {
  color: var(--text-tertiary);
  cursor: not-allowed;
}

.live-graph__divider {
  width: var(--border-1);
  height: var(--space-4);
  background: var(--border-default);
}

.live-graph__canvas {
  width: 100%;
  min-height: 420px;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  background: var(--surface-base);
  overflow: hidden;
}

.live-graph__journey {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
}

.live-graph__journey-summary {
  color: var(--text-secondary);
}

.live-graph__empty {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
