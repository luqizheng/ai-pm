<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import PageFrame from "@/components/shell/PageFrame.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import StatCard from "@/components/ui/stat-card/StatCard.vue";
import { listTasks } from "@/api/tasks";
import type { Task } from "@/api/types";
import { getHumanAttentionQueue, listTaskRuns } from "@/api/execution-plans";
import type { TaskRun, HumanAttentionQueue } from "@/api/execution-plans";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import {
  schedulingCondition,
  type SchedulingCondition
} from "@/views/project/tasks/tasks-contract";
import { describeError } from "@/lib/errors";

/**
 * PA-07 项目执行看板（Figma 179:3603 00「Project Dashboard」）四块：
 *   1. Execution overview —— 任务态统计（13 态聚合派生）；
 *   2. Needs human attention —— getHumanAttentionQueue（waiting/interactions/approvals）；
 *   3. Currently executing —— run 通道活跃（queued/claimed/running）；
 *   4. Waiting and ready —— 调度条件派生分组（不改任务状态）。
 * 数据全部前端拼合既有端点，零新增后端（计划裁决）。
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

type TaskLite = Task;

const tasks = ref<TaskLite[]>([]);
const runs = ref<TaskRun[]>([]);
const attention = ref<HumanAttentionQueue | null>(null);
const loading = ref(false);
const loadError = ref<string | null>(null);

function taskCondition(task: TaskLite): SchedulingCondition {
  return schedulingCondition({
    status: task.status,
    product_blocked: task.product_blocked,
    deps_total: task.deps_total,
    deps_met: task.deps_met,
    latest_run: task.latest_run
  });
}

const IN_CHANNEL = [
  "claimed",
  "running",
  "awaiting_decision",
  "submitted",
  "reviewing",
  "changes_requested"
];

const statActive = computed(
  () => tasks.value.filter(task => IN_CHANNEL.includes(task.status)).length
);
const statWaiting = computed(
  () => tasks.value.filter(task => taskCondition(task).key === "waiting").length
);
const statReady = computed(
  () => tasks.value.filter(task => taskCondition(task).key === "ready").length
);
const statAttention = computed(() => {
  if (!attention.value) return 0;
  return (
    attention.value.waiting.length +
    attention.value.interactions.length +
    attention.value.approvals.length
  );
});

const activeRuns = computed(() =>
  runs.value.filter(run =>
    ["queued", "claimed", "running"].includes(run.status)
  )
);

const waitingTasks = computed(() =>
  tasks.value
    .filter(task => taskCondition(task).key === "waiting")
    .map(task => ({ task, condition: taskCondition(task) }))
);
const readyTasks = computed(() =>
  tasks.value
    .filter(task => taskCondition(task).key === "ready")
    .map(task => ({ task, condition: taskCondition(task) }))
);

const attentionItems = computed(() => {
  if (!attention.value)
    return [] as Array<{ kind: string; title: string; id: string }>;
  return [
    ...attention.value.approvals.map(item => ({
      kind: "approval",
      title: item.task_title ?? item.kind,
      id: item.task_id ?? item.id
    })),
    ...attention.value.interactions.map(item => ({
      kind: "interaction",
      title: item.title,
      id: item.task_id
    })),
    ...attention.value.waiting.map(item => ({
      kind: "waiting",
      title: item.task_title ?? item.reason,
      id: item.task_id
    }))
  ];
});

async function load() {
  if (!projectId.value) {
    tasks.value = [];
    runs.value = [];
    attention.value = null;
    return;
  }
  loading.value = true;
  loadError.value = null;
  try {
    const [taskRes, runRes, queue] = await Promise.all([
      listTasks({ project_id: projectId.value, size: 200 }),
      listTaskRuns({ project_id: projectId.value, size: 100 }),
      getHumanAttentionQueue(projectId.value)
    ]);
    tasks.value = taskRes.items ?? [];
    runs.value = runRes.items ?? [];
    attention.value = queue;
  } catch (error) {
    loadError.value = describeError(error);
  } finally {
    loading.value = false;
  }
}

onMounted(() => void load());
watch(projectId, () => void load());

function goTasks() {
  void router.push("/project/tasks/list");
}

function goTask(id: string) {
  void router.push(`/project/tasks/${id}`);
}
</script>

<template>
  <PageFrame
    :title="t('pa07.dashboard.title')"
    :description="t('pa07.dashboard.subtitle')"
  >
    <p v-if="loadError" class="ex-error">{{ loadError }}</p>
    <p v-else-if="loading" class="ex-note">{{ t("common.loading") }}</p>

    <template v-else>
      <!-- 1 · Execution overview -->
      <section class="ex-block" data-testid="dash-overview">
        <p class="ex-block__title">{{ t("pa07.dashboard.overviewTitle") }}</p>
        <div v-if="tasks.length === 0" class="ex-empty">
          {{ t("pa07.dashboard.emptyOverview") }}
        </div>
        <div v-else class="ex-stats">
          <StatCard
            :label="t('pa07.dashboard.statActive')"
            :value="String(statActive)"
            data-testid="dash-stat-active"
          />
          <StatCard
            :label="t('pa07.dashboard.statWaiting')"
            :value="String(statWaiting)"
            data-testid="dash-stat-waiting"
          />
          <StatCard
            :label="t('pa07.dashboard.statReady')"
            :value="String(statReady)"
            data-testid="dash-stat-ready"
          />
          <StatCard
            :label="t('pa07.dashboard.statAttention')"
            :value="String(statAttention)"
            data-testid="dash-stat-attention"
          />
        </div>
      </section>

      <!-- 2 · Needs human attention -->
      <section class="ex-block" data-testid="dash-attention">
        <p class="ex-block__title">{{ t("pa07.dashboard.attentionTitle") }}</p>
        <div v-if="attentionItems.length === 0" class="ex-empty">
          {{ t("pa07.dashboard.emptyAttention") }}
        </div>
        <ul v-else class="ex-list">
          <li
            v-for="item in attentionItems"
            :key="`${item.kind}-${item.id}`"
            class="ex-item"
          >
            <Badge
              :variant="item.kind === 'approval' ? 'warning' : 'accent'"
              :outline="true"
            >
              {{ item.kind }}
            </Badge>
            <button
              class="ex-item__title"
              data-test="dash-attention-item"
              @click="goTask(item.id)"
            >
              {{ item.title }}
            </button>
          </li>
        </ul>
      </section>

      <!-- 3 · Currently executing -->
      <section class="ex-block" data-testid="dash-executing">
        <p class="ex-block__title">{{ t("pa07.dashboard.executingTitle") }}</p>
        <div v-if="activeRuns.length === 0" class="ex-empty">
          {{ t("pa07.dashboard.emptyExecuting") }}
        </div>
        <ul v-else class="ex-list">
          <li v-for="run in activeRuns" :key="run.id" class="ex-item">
            <Badge variant="info">Run #{{ run.attempt_no }}</Badge>
            <button
              class="ex-item__title"
              data-test="dash-run-item"
              @click="goTask(run.task_id)"
            >
              {{ run.task_title }} · {{ run.status }}
            </button>
          </li>
        </ul>
      </section>

      <!-- 4 · Waiting and ready -->
      <section class="ex-block" data-testid="dash-waiting">
        <p class="ex-block__title">{{ t("pa07.dashboard.waitingTitle") }}</p>
        <div
          v-if="waitingTasks.length === 0 && readyTasks.length === 0"
          class="ex-empty"
        >
          {{ t("pa07.dashboard.emptyWaiting") }}
        </div>
        <template v-else>
          <ul class="ex-list">
            <li
              v-for="{ task, condition } in [
                ...waitingTasks,
                ...readyTasks
              ].slice(0, 8)"
              :key="task.id"
              class="ex-item"
            >
              <span class="ex-item__code">{{ task.code }}</span>
              <button
                class="ex-item__title"
                data-test="dash-waiting-item"
                @click="goTask(task.id)"
              >
                {{ task.title }}
              </button>
              <span
                class="ex-item__condition"
                :class="{ 'is-danger': condition.key === 'blocked_product' }"
              >
                {{ condition.label }}
                <template v-if="condition.detail">
                  · {{ condition.detail }}</template
                >
              </span>
            </li>
          </ul>
        </template>
        <div class="ex-foot">
          <Button
            variant="secondary"
            size="sm"
            data-test="dash-go-tasks"
            @click="goTasks"
          >
            {{ t("pa07.dashboard.goTasks") }}
          </Button>
        </div>
      </section>
    </template>
  </PageFrame>
</template>

<style scoped>
.ex-error {
  margin: 0;
  color: var(--ws-color-danger-text);
}

.ex-note {
  margin: 0;
  color: var(--ws-text-secondary);
}

.ex-block {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-base);
}

.ex-block__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.ex-stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: var(--space-3);
}

.ex-empty {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.ex-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ex-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.ex-item__code {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-disabled);
}

.ex-item__title {
  padding: 0;
  border: none;
  background: none;
  text-align: left;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
  cursor: pointer;
}

.ex-item__condition {
  margin-left: auto;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.ex-item__condition.is-danger {
  color: var(--ws-color-danger-text);
}

.ex-foot {
  display: flex;
  justify-content: flex-end;
}
</style>
