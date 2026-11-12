<script setup lang="ts">
/**
 * VisualizationsView —— 项目图形产物工作区（V0-8 主视图）。
 *
 * 布局：槽位列表（左）+ 当前产物（产物通道 + 表格视图 + 溯源侧栏）。
 *
 * 纪律：
 * - 页面根必须是 `PageFrame`（ai-frontend/AGENTS.md）；
 * - 项目上下文走 `?project_id=` 钥匙模式（query 优先 → 回退 scope，
 *   排除 `SCOPE_ALL` 哨兵——必须用常量比对）；
 * - 权限裁剪在服务端完成，前端只渲染返回内容；
 * - P0 产物为静态图，节点级交互经表格视图与溯源面板访问。
 */
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { PageFrame } from "@/components/shell";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import {
  getArtifact,
  listArtifacts,
  listSlots,
  type VizSlotSummary
} from "@/api/visualizations";
import type { VizArtifact, VizNode } from "@/lib/viz/types";
import VizArtifactFrame from "@/components/viz/VizArtifactFrame.vue";
import VizAiExplanation from "@/components/viz/VizAiExplanation.vue";
import VizEmptyState from "@/components/viz/VizEmptyState.vue";
import VizFeedbackArea from "@/components/viz/VizFeedbackArea.vue";
import VizSpecTable from "@/components/viz/VizSpecTable.vue";
import VizTracePanel from "@/components/viz/VizTracePanel.vue";
import VizVersionBar from "@/components/viz/VizVersionBar.vue";

const { t } = useI18n();
const route = useRoute();
const scope = useScopeStore();

const slots = ref<VizSlotSummary[]>([]);
const artifact = ref<VizArtifact | null>(null);
const loading = ref(false);
const failed = ref(false);
const traceOpen = ref(false);
const traceNode = ref<VizNode | null>(null);
const history = ref<VizArtifact[]>([]);
const compareHint = ref<string | null>(null);
const feedbackText = ref<string | null>(null);

/** 项目 id：query 优先 → 回退 scope（排除哨兵）。 */
const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  const current = scope.currentProjectId;
  return current && current !== SCOPE_ALL ? current : "";
});

async function load(): Promise<void> {
  if (!projectId.value) {
    slots.value = [];
    artifact.value = null;
    return;
  }
  loading.value = true;
  failed.value = false;
  try {
    const response = await listSlots(projectId.value);
    slots.value = response.items ?? [];
    const first = slots.value[0];
    artifact.value = first ? await getArtifact(first.artifact_id) : null;
  } catch {
    failed.value = true;
    slots.value = [];
    artifact.value = null;
  } finally {
    loading.value = false;
  }
}

async function selectSlot(slot: VizSlotSummary): Promise<void> {
  try {
    artifact.value = await getArtifact(slot.artifact_id);
    history.value = await loadHistory(slot.slot_key);
    compareHint.value = null;
  } catch {
    failed.value = true;
  }
}

/** 取槽位历史版本（用于版本栏与对比）。 */
async function loadHistory(slotKey: string): Promise<VizArtifact[]> {
  try {
    const response = await listArtifacts(projectId.value, {
      slot_key: slotKey,
      size: 20
    });
    return (response.items ?? []).filter(a => a.id !== artifact.value?.id);
  } catch {
    return [];
  }
}

/** 切换到指定版本。 */
async function selectVersion(artifactId: string): Promise<void> {
  try {
    artifact.value = await getArtifact(artifactId);
  } catch {
    failed.value = true;
  }
}

/** 版本对比：展示两版之间的对象差异概要（结构化 diff，非像素对比）。 */
function compareVersions(leftId: string, rightId: string): void {
  const left = allVersions.value.find(a => a.id === leftId);
  const right = allVersions.value.find(a => a.id === rightId);
  if (!left || !right) return;
  compareHint.value = t("viz.version.diffSummary", {
    left: left.version_no,
    right: right.version_no,
    nodes: right.spec.nodes.length - left.spec.nodes.length,
    edges: right.spec.edges.length - left.spec.edges.length
  });
}

/** 全部版本（当前 + 历史），供对比查找。 */
const allVersions = computed(() =>
  artifact.value ? [artifact.value, ...history.value] : []
);

/** 反馈：主视图内不直接触发生成——回到对话上下文执行（V0-5 链路）。 */
function onFeedback(text: string): void {
  feedbackText.value = text;
}

function openTrace(node: VizNode): void {
  traceNode.value = node;
  traceOpen.value = true;
}

watch(projectId, () => {
  void load();
});
onMounted(() => {
  void load();
});
</script>

<template>
  <PageFrame :title="t('viz.tab.title')">
    <div class="viz-view">
      <aside class="viz-view__slots">
        <p v-if="loading" class="viz-view__muted">{{ t("viz.tab.loading") }}</p>
        <p v-else-if="failed" class="viz-view__muted">
          {{ t("viz.tab.loadFailed") }}
        </p>
        <VizEmptyState v-else-if="slots.length === 0" />
        <ul v-else class="viz-view__slot-list">
          <li v-for="slot in slots" :key="slot.slot_key">
            <button
              type="button"
              class="viz-view__slot"
              :data-testid="'viz-slot-button'"
              @click="selectSlot(slot)"
            >
              {{ slot.slot_key }}
            </button>
          </li>
        </ul>
      </aside>

      <section class="viz-view__main">
        <template v-if="artifact">
          <VizVersionBar
            :artifact="artifact"
            :history="history"
            :explanatory="artifact.nature === 'explanatory'"
            @select-version="selectVersion"
            @compare="compareVersions"
          />
          <p
            v-if="compareHint"
            class="viz-view__compare"
            :data-testid="'viz-compare-hint'"
          >
            {{ compareHint }}
          </p>
          <VizArtifactFrame
            :artifact="artifact"
            actionable
            @accept="onFeedback(t('viz.artifact.accepted'))"
            @reject="onFeedback(t('viz.artifact.rejected'))"
            @regenerate="onFeedback(t('viz.stale.regenerate'))"
          />
          <VizSpecTable :spec="artifact.spec" @select-node="openTrace" />
          <VizFeedbackArea @feedback="onFeedback" />
          <p
            v-if="feedbackText"
            class="viz-view__compare"
            :data-testid="'viz-feedback-hint'"
          >
            {{ feedbackText }}
          </p>
        </template>
        <VizEmptyState v-else :failed="failed" @action="load" />
      </section>
    </div>

    <VizTracePanel
      v-if="artifact"
      :open="traceOpen"
      :spec="artifact.spec"
      :node="traceNode"
      @update:open="traceOpen = $event"
    />
  </PageFrame>
</template>

<style scoped>
.viz-view {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--space-4);
  align-items: start;
}

.viz-view__slots {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.viz-view__slot-list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.viz-view__slot {
  width: 100%;
  text-align: left;
  padding: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.viz-view__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.viz-view__muted {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
