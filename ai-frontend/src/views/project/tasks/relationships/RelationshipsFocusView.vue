<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import PageFrame from "@/components/shell/PageFrame.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import TasksFrame from "../TasksFrame.vue";
import { getDependencyGraph, type DependencyGraph } from "@/api/task-plans";
import { aggregateStatusLabel, aggregateStatusTone } from "../tasks-contract";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";

/**
 * PA-07 依赖关系焦点视图（Figma 179:3603 C「Dependency Relationships」）。
 *
 * 三列焦点卡：Depends On → Selected Task → Blocks。数据 = 项目级依赖图
 * （dependency-graph，后端已完成分层/环检测，本页只做焦点投影）；
 * 选中任务经 URL query ?focus= 深链。AND 门禁说明卡随选中任务固定呈现。
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

const graph = ref<DependencyGraph | null>(null);
const loading = ref(false);
const loadError = ref<string | null>(null);
const focusId = ref<string>("");

const focusCandidates = computed(() => graph.value?.nodes ?? []);
const focusNode = computed(
  () => focusCandidates.value.find(node => node.id === focusId.value) ?? null
);

const upstreamEdges = computed(
  () => (graph.value?.edges ?? []).filter(edge => edge.to_task_id === focusId.value)
);
const downstreamEdges = computed(
  () => (graph.value?.edges ?? []).filter(edge => edge.from_task_id === focusId.value)
);

function nodeOf(id: string) {
  return focusCandidates.value.find(node => node.id === id) ?? null;
}

/** 焦点候选：有边关系的节点优先（无图数据时允许从全量节点选）。 */
const selectable = computed(() => focusCandidates.value);

function selectFocus(id: string) {
  focusId.value = id;
  void router.replace({ query: { ...route.query, focus: id } });
}

async function load() {
  if (!projectId.value) {
    graph.value = null;
    return;
  }
  loading.value = true;
  loadError.value = null;
  try {
    graph.value = await getDependencyGraph(projectId.value);
    const fromQuery = typeof route.query.focus === "string" ? route.query.focus : "";
    focusId.value =
      fromQuery && focusCandidates.value.some(node => node.id === fromQuery)
        ? fromQuery
        : (focusCandidates.value[0]?.id ?? "");
  } catch (error) {
    loadError.value = describeError(error);
    graph.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(() => void load());
watch(projectId, () => void load());
watch(
  () => route.query.focus,
  value => {
    if (typeof value === "string" && value) focusId.value = value;
  }
);

function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

function openTask(id: string) {
  void router.push(`/project/tasks/${id}`);
}
</script>

<template>
  <PageFrame :title="t('pa07.relationships.title')" :description="t('pa07.relationships.subtitle')">
    <TasksFrame />

    <div class="rf-toolbar">
      <RouterLink
        to="/project/tasks/list"
        class="rf-toolbar__tab"
        data-testid="rel-view-list"
      >
        {{ t("pa07.list.viewList") }}
      </RouterLink>
      <RouterLink
        to="/project/tasks/relationships"
        class="rf-toolbar__tab is-active"
        aria-current="page"
        data-testid="rel-view-relationships"
      >
        {{ t("pa07.list.viewRelationships") }}
      </RouterLink>
      <label class="rf-focus">
        <span class="rf-focus__label">{{ t("pa07.relationships.selectedTask") }}</span>
        <select
          class="rf-focus__select"
          data-test="rel-focus-select"
          :value="focusId"
          @change="selectFocus(($event.target as HTMLSelectElement).value)"
        >
          <option v-if="selectable.length === 0" value="">—</option>
          <option v-for="node in selectable" :key="node.id" :value="node.id">
            {{ node.title }}
          </option>
        </select>
      </label>
      <Button
        variant="primary"
        size="md"
        data-test="rel-add-dependency"
        :disabled="!focusId"
        @click="openTask(focusId)"
      >
        {{ t("pa07.relationships.addDependency") }}
      </Button>
    </div>

    <p v-if="loadError" class="rf-error">{{ loadError }}</p>
    <p v-else-if="loading" class="rf-note">{{ t("common.loading") }}</p>
    <EmptyState
      v-else-if="!focusNode"
      :title="t('pa07.list.empty')"
      data-test="rel-empty"
    />

    <template v-else>
      <div class="rf-gate" data-test="rel-and-gate">
        <p class="rf-gate__title">{{ t("pa07.relationships.andGateTitle") }}</p>
        <p class="rf-gate__copy">{{ t("pa07.relationships.andGateCopy") }}</p>
      </div>

      <div class="rf-grid">
        <section class="rf-col" data-testid="rel-depends-on">
          <p class="rf-col__caption">{{ t("pa07.relationships.dependsOn") }}</p>
          <div v-if="upstreamEdges.length === 0" class="rf-card rf-card--muted" data-test="rel-depends-empty">
            {{ t("pa07.relationships.emptyDependsOn") }}
          </div>
          <button
            v-for="edge in upstreamEdges"
            :key="edge.from_task_id"
            class="rf-card rf-card--click"
            data-test="rel-upstream-card"
            @click="openTask(edge.from_task_id)"
          >
            <span class="rf-card__title">{{ nodeOf(edge.from_task_id)?.title ?? "—" }}</span>
            <span
              class="rf-card__status"
              :class="`rf-card__status--${aggregateStatusTone(nodeOf(edge.from_task_id)?.status ?? '')}`"
            >
              {{ aggregateStatusLabel(nodeOf(edge.from_task_id)?.status ?? "") }}
            </span>
            <span class="rf-card__edge">{{ edge.dependency_type }}</span>
          </button>
        </section>

        <div class="rf-arrow" aria-hidden="true">→</div>

        <section class="rf-col" data-testid="rel-selected">
          <p class="rf-col__caption">{{ t("pa07.relationships.selectedTask") }}</p>
          <div class="rf-card rf-card--focus">
            <span class="rf-card__title">{{ focusNode.title }}</span>
            <span
              class="rf-card__status"
              :class="`rf-card__status--${aggregateStatusTone(focusNode.status)}`"
            >
              {{ aggregateStatusLabel(focusNode.status) }}
            </span>
            <span class="rf-card__meta">
              {{ t("pa07.relationships.dependsOn") }} {{ upstreamEdges.length }} ·
              {{ t("pa07.relationships.blocks") }} {{ downstreamEdges.length }}
            </span>
            <Button
              variant="secondary"
              size="sm"
              data-test="rel-task-details"
              @click="openTask(focusNode.id)"
            >
              {{ t("pa07.relationships.taskDetails") }}
            </Button>
          </div>
        </section>

        <div class="rf-arrow" aria-hidden="true">→</div>

        <section class="rf-col" data-testid="rel-blocks">
          <p class="rf-col__caption">{{ t("pa07.relationships.blocks") }}</p>
          <div v-if="downstreamEdges.length === 0" class="rf-card rf-card--muted" data-test="rel-blocks-empty">
            {{ t("pa07.relationships.emptyBlocks") }}
          </div>
          <button
            v-for="edge in downstreamEdges"
            :key="edge.to_task_id"
            class="rf-card rf-card--click"
            data-test="rel-downstream-card"
            @click="openTask(edge.to_task_id)"
          >
            <span class="rf-card__title">{{ nodeOf(edge.to_task_id)?.title ?? "—" }}</span>
            <span
              class="rf-card__status"
              :class="`rf-card__status--${aggregateStatusTone(nodeOf(edge.to_task_id)?.status ?? '')}`"
            >
              {{ aggregateStatusLabel(nodeOf(edge.to_task_id)?.status ?? "") }}
            </span>
            <span class="rf-card__edge">{{ edge.dependency_type }}</span>
          </button>
        </section>
      </div>

      <div v-if="downstreamEdges.length > 1" class="rf-parallel" data-test="rel-parallel">
        <Badge variant="accent">{{ t("pa07.relationships.parallelTitle") }}</Badge>
        <p class="rf-parallel__copy">{{ t("pa07.relationships.parallelCopy") }}</p>
      </div>
    </template>
  </PageFrame>
</template>

<style scoped>
.rf-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  padding-bottom: var(--space-3);
}

.rf-toolbar__tab {
  display: inline-flex;
  align-items: center;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-text-secondary);
  text-decoration: none;
}

.rf-toolbar__tab.is-active {
  color: var(--ws-text-primary);
  box-shadow: inset 0 calc(-1 * var(--size-indicator)) var(--ws-accent);
}

.rf-focus {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-left: auto;
}

.rf-focus__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.rf-focus__select {
  min-width: 220px;
  height: var(--size-control-md);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  background: var(--surface-base);
  color: var(--ws-text-primary);
}

.rf-error {
  margin: 0;
  color: var(--ws-color-danger-text);
}

.rf-note {
  margin: 0;
  color: var(--ws-text-secondary);
}

.rf-gate {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-raised);
}

.rf-gate__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.rf-gate__copy {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.rf-grid {
  display: grid;
  grid-template-columns: 1fr auto 1fr auto 1fr;
  gap: var(--space-4);
  align-items: start;
}

.rf-col {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rf-col__caption {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: 500;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--ws-text-secondary);
}

.rf-arrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  color: var(--ws-text-disabled);
  padding-top: var(--space-8);
}

.rf-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-1);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-base);
  text-align: left;
}

.rf-card--click {
  cursor: pointer;
}

.rf-card--focus {
  background: var(--surface-raised);
}

.rf-card--muted {
  color: var(--ws-text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.rf-card__title {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 500;
  color: var(--ws-text-primary);
}

.rf-card__status {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.rf-card__status--success {
  color: var(--ws-color-success-text);
}

.rf-card__status--danger {
  color: var(--ws-color-danger-text);
}

.rf-card__status--info {
  color: var(--ws-color-info-text);
}

.rf-card__status--warning {
  color: var(--ws-color-warning-text);
}

.rf-card__status--muted,
.rf-card__status--neutral {
  color: var(--ws-text-secondary);
}

.rf-card__edge {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-disabled);
}

.rf-card__meta {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.rf-parallel {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-base);
}

.rf-parallel__copy {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
