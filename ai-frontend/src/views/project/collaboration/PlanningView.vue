<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import { listTasks } from "@/api/pm";
import { markTasksReady } from "@/api/tasks";
import { getDependencyGraph, type DependencyGraphNode } from "@/api/task-plans";
import { fetchProjectFeed } from "@/api/feeds";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 E — Task planning & dependencies（Figma 259:486，Plan 页签）。
 *
 * 「并行工作，显式依赖」：依赖分层图（dependency-graph 真实分层/关键路径
 * /并行组，节点=code·优先级/标题/状态）+ 计划调整活动（项目 feed 真实条目）
 * + Start authorized execution（E5 整批 draft→ready 人类确认入口，
 * markTasksReady 真端点——ready 化是 E2 真人闸，AI 永不置 ready）。
 * 右栏 Plan rationale（依赖规则/并行非独立/升级边界——PA-06-04 基线文案）。
 *
 * 诚实缺位：画布「PLAN v6」版本化计划史无读面——历史段呈现项目活动而非
 * 编造版本号；edge 标签（Accepted v1 等）不实现。
 */
const { t } = useI18n();
const router = useRouter();
const toast = useToast();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);
const nodes = ref<DependencyGraphNode[]>([]);
const layerIds = ref<string[][]>([]);
const parallelCount = ref(0);
const criticalCount = ref(0);
const draftTaskIds = ref<string[]>([]);
const activities = ref<
  Array<{ id: string; summary: string; created_at: string }>
>([]);
const starting = ref(false);

function layerOf(nodeId: string): number {
  return layerIds.value.findIndex(layer => layer.includes(nodeId));
}

const layerGroups = computed(() => {
  const byNode = new Map(nodes.value.map(node => [node.id, node]));
  return layerIds.value
    .map(layer =>
      layer
        .map(id => byNode.get(id))
        .filter((n): n is DependencyGraphNode => Boolean(n))
    )
    .filter(layer => layer.length > 0);
});

function isWaiting(node: DependencyGraphNode): boolean {
  return node.status === "blocked" || node.status === "waiting";
}

function nodeStatus(node: DependencyGraphNode): string {
  return node.status;
}

const planMeta = computed(() => {
  const total = nodes.value.length;
  if (total === 0) return "";
  return t("pa06.plan.summary", {
    total,
    parallel: parallelCount.value,
    critical: criticalCount.value
  });
});

async function load(): Promise<void> {
  if (!hasProject.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const id = projectId.value;
    const [graph, tasks, feed] = await Promise.all([
      getDependencyGraph(id),
      listTasks({ project_id: id, page: 1, size: 200 }),
      fetchProjectFeed(id, 4).catch(() => [])
    ]);
    nodes.value = graph.nodes ?? [];
    layerIds.value = graph.layers ?? [];
    parallelCount.value = graph.parallel_groups?.length ?? 0;
    criticalCount.value = graph.critical_path?.length ?? 0;
    draftTaskIds.value = (tasks.items ?? [])
      .filter(task => task.status === "draft")
      .map(task => task.id);
    activities.value = feed.map(row => ({
      id: row.id,
      summary: row.summary,
      created_at: row.created_at
    }));
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

/** E5/DK-0：整批 draft→ready 的人类确认入口（真端点，E2 真人闸） */
async function startAuthorizedExecution(): Promise<void> {
  if (starting.value || draftTaskIds.value.length === 0) return;
  starting.value = true;
  try {
    await markTasksReady(projectId.value, draftTaskIds.value);
    toast.toast({
      title: t("pa06.plan.readyToast", { count: draftTaskIds.value.length })
    });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    starting.value = false;
  }
}

function reviewScopeChange(): void {
  void router.push("/project/collaboration/decisions");
}

function inspectAuthorization(): void {
  void router.push("/project/collaboration/authorization");
}

onMounted(load);
watch(projectId, load);
</script>

<template>
  <CollaborationFrame>
    <ScreenFeedback
      v-if="loading"
      state="loading"
      :title="$t('pa06.plan.loadingEyebrow')"
    />
    <ScreenFeedback
      v-else-if="error"
      state="error"
      :description="error"
      @retry="load"
    />
    <template v-else>
      <template v-if="!hasProject">
        <ScreenFeedback state="empty" :title="$t('pa06.frame.noProject')" />
      </template>
      <ScreenFeedback
        v-else-if="nodes.length === 0"
        state="empty"
        :title="$t('pa06.plan.emptyEyebrow')"
        :description="$t('pa06.plan.emptyHint')"
      />
      <CollabLayout v-else>
        <div class="pl-head">
          <div class="pl-head__eyebrow-row">
            <span class="pl-eyebrow">{{ $t("pa06.plan.eyebrow") }}</span>
            <Badge variant="accent">{{ $t("pa06.plan.scopeBadge") }}</Badge>
          </div>
          <h2 class="pl-head__title">{{ $t("pa06.plan.headline") }}</h2>
          <p class="pl-head__meta">{{ planMeta }}</p>
        </div>

        <!-- 依赖分层图 -->
        <div class="pl-card" data-testid="pl-graph">
          <span class="pl-eyebrow">{{ $t("pa06.plan.graphEyebrow") }}</span>
          <div
            v-for="(layer, index) in layerGroups"
            :key="index"
            class="pl-layer"
            :data-testid="`pl-layer-${index}`"
          >
            <span class="pl-layer__label">
              {{ $t("pa06.plan.layerLabel", { layer: index + 1 }) }}
            </span>
            <div class="pl-layer__nodes">
              <div
                v-for="node in layer"
                :key="node.id"
                class="pl-node"
                :class="{ 'is-waiting': isWaiting(node) }"
                :data-testid="`pl-node-${node.id}`"
              >
                <p class="pl-node__code">
                  {{ node.title.split(" ")[0] }} · P{{ node.priority }}
                </p>
                <p class="pl-node__title">{{ node.title }}</p>
                <p class="pl-node__meta">{{ nodeStatus(node) }}</p>
              </div>
            </div>
          </div>
        </div>

        <!-- 计划调整活动（真实 feed 条目；画布 vN 版本史无读面=诚实替代） -->
        <span class="pl-eyebrow">{{ $t("pa06.plan.historyEyebrow") }}</span>
        <div class="pl-history" data-testid="pl-history">
          <p v-for="row in activities" :key="row.id" class="pl-history__line">
            <span class="pl-history__time">{{
              new Date(row.created_at).toLocaleTimeString()
            }}</span>
            <span>{{ row.summary }}</span>
          </p>
          <p v-if="activities.length === 0" class="pl-history__line">
            {{ $t("pa06.plan.noHistory") }}
          </p>
          <p class="pl-history__note">{{ $t("pa06.plan.escalationNote") }}</p>
        </div>

        <div class="pl-actions">
          <Button
            variant="primary"
            size="sm"
            :disabled="starting || draftTaskIds.length === 0"
            data-testid="pl-start"
            @click="startAuthorizedExecution"
          >
            {{
              draftTaskIds.length > 0
                ? $t("pa06.plan.startAction", { count: draftTaskIds.length })
                : $t("pa06.plan.startNone")
            }}
          </Button>
          <Button variant="secondary" size="sm" @click="reviewScopeChange">
            {{ $t("pa06.plan.reviewScope") }}
          </Button>
        </div>

        <template #context>
          <CollabContextPanel
            :title="$t('pa06.plan.ctxTitle')"
            :badge="$t('pa06.plan.ctxBadge')"
          >
            <CollabContextSection :label="$t('pa06.plan.ctxRules')">
              {{ $t("pa06.plan.ctxRulesText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.plan.ctxParallel')">
              {{ $t("pa06.plan.ctxParallelText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.plan.ctxEscalation')">
              {{ $t("pa06.plan.ctxEscalationText") }}
            </CollabContextSection>
            <Button variant="secondary" size="sm" @click="inspectAuthorization">
              {{ $t("pa06.plan.ctxInspect") }}
            </Button>
            <p class="pl-ctx-note">{{ $t("pa06.plan.ctxNote") }}</p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.pl-eyebrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.pl-head {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
}

.pl-head__eyebrow-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: var(--space-3);
}

.pl-head__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: 1.375rem;
  font-weight: var(--weight-semibold);
  letter-spacing: -0.018em;
  color: var(--text-primary);
}

.pl-head__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.pl-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.pl-layer {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
}

.pl-layer__label {
  flex: 0 0 5.5rem;
  padding-top: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.pl-layer__nodes {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.pl-node {
  width: 13rem;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-2) var(--space-3);
}

.pl-node.is-waiting {
  background: color-mix(in srgb, var(--ws-accent) 12%, transparent);
  border-color: var(--ws-accent);
}

.pl-node__code {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  color: var(--text-secondary);
}

.pl-node__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.pl-node__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.pl-history {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.pl-history__line {
  margin: 0;
  display: flex;
  gap: var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.pl-history__time {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.pl-history__note {
  margin: var(--space-1) 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.pl-actions {
  display: flex;
  gap: var(--space-2);
}

.pl-ctx-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: 1.45;
  color: var(--text-secondary);
}
</style>
