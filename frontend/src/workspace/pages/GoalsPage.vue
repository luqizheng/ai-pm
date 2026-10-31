<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { GitBranch } from "lucide-vue-next";
import PageHeader from "./PageHeader.vue";
import { goals, type MilestoneStatus, type TaskStatus } from "../fixtures";
import {
  buildPageGoals,
  milestoneStatusOf,
  principalNameMap,
  type PageGoal
} from "./goals-model";
import { listMilestones, listPrincipals, listTasks } from "@/api/pm";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import { formatDate } from "@/api/format";
import type { Principal, Task } from "@/api/types";
import type { Milestone as ApiMilestone } from "@/api/workspace-view";

const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const scope = useScopeStore();

/** mock 模式用演示数据；真后端模式按项目读里程碑 + 任务。单测一律走 fixtures。 */
const useMock =
  import.meta.env.VITE_USE_MOCK === "1" || import.meta.env.MODE === "test";

/** V1.2：三种可切换视图（目标树 / 依赖图 / 时间线） */
const view = ref<"tree" | "deps" | "timeline">("tree");

const views: {
  key: "tree" | "deps" | "timeline";
  labelKey: string;
  test: string;
}[] = [
  {
    key: "tree",
    labelKey: "workspace.pages.goalsExtra.viewTree",
    test: "view-tree"
  },
  {
    key: "deps",
    labelKey: "workspace.pages.goalsExtra.viewDeps",
    test: "view-deps"
  },
  {
    key: "timeline",
    labelKey: "workspace.pages.goalsExtra.viewTimeline",
    test: "view-timeline"
  }
];

const msLabel: Record<MilestoneStatus, string> = {
  done: "workspace.pages.goals.msDone",
  active: "workspace.pages.goals.msActive",
  pending: "workspace.pages.goals.msPending"
};

const msClass: Record<MilestoneStatus, string> = {
  done: "is-done",
  active: "is-active",
  pending: "is-pending"
};

const taskLabel: Record<TaskStatus, string> = {
  done: "workspace.pages.goals.taskDone",
  doing: "workspace.pages.goals.taskDoing",
  todo: "workspace.pages.goals.taskTodo",
  blocked: "workspace.pages.goalsExtra.taskBlocked",
  "awaiting-acceptance": "workspace.pages.goalsExtra.taskAwaiting",
  proposed: "workspace.pages.goalsExtra.proposed",
  cancelled: "workspace.pages.goalsExtra.taskCancelled"
};

const taskClass: Record<TaskStatus, string> = {
  done: "task-done",
  doing: "task-doing",
  todo: "task-todo",
  blocked: "task-blocked",
  "awaiting-acceptance": "task-awaiting",
  proposed: "task-proposed",
  cancelled: "task-cancelled"
};

const allMilestones = goals.flatMap(goal =>
  goal.milestones.map(ms => ({ ...ms, goalCode: goal.code }))
);

/* ---------------------------------------------------------------- 真后端模式 */


const loading = ref(false);
const failed = ref(false);
const apiMilestones = ref<ApiMilestone[]>([]);
const apiTasks = ref<Task[]>([]);
const apiPrincipals = ref<Principal[]>([]);

/** 项目来源：显式 scope 选择 > 路由里是真实 UUID 时用它；否则提示先选项目。 */
const projectId = computed<string | undefined>(() => {
  if (scope.projectIdParam) return scope.projectIdParam;
  const param = String(route.params.projectId ?? "");
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(param)
  ) {
    return param;
  }
  return undefined;
});

const selectedProjectId = computed({
  get: () => (scope.currentProjectId === SCOPE_ALL ? "" : scope.currentProjectId),
  set: (value: string) => {
    scope.currentProjectId = value || SCOPE_ALL;
  }
});

const principalNames = computed<Record<string, string>>(() =>
  principalNameMap(apiPrincipals.value)
);

const pageGoals = computed<PageGoal[]>(() => {
  if (useMock) return goals;
  return buildPageGoals(
    apiMilestones.value,
    apiTasks.value,
    principalNames.value,
    {
      noPlannedDate: String(t("workspace.pages.goalsExtra.noPlannedDate")),
      unassigned: String(t("workspace.pages.goalsExtra.unassigned")),
      basisByTasks: (done, total) =>
        String(
          t("workspace.pages.goalsExtra.basisByTasks", { done, total })
        ),
      formatPlannedAt: formatDate
    }
  );
});

const timelineItems = computed(() => {
  if (useMock) return allMilestones;
  return pageGoals.value.map(goal => ({
    id: goal.id,
    name: goal.name,
    status: milestoneStatusOf(goal),
    due: goal.selfDue ?? "—",
    goalCode: goal.code
  }));
});

const hasDependencies = computed(() =>
  pageGoals.value.some(goal => goal.dependencies.length > 0)
);

async function loadGoals() {
  if (useMock) return;
  failed.value = false;
  if (!projectId.value) {
    apiMilestones.value = [];
    apiTasks.value = [];
    return;
  }
  loading.value = true;
  try {
    const [milestones, tasks, principals] = await Promise.all([
      listMilestones({ project_id: projectId.value, page: 1, size: 200 }),
      listTasks({ project_id: projectId.value, page: 1, size: 200 }),
      listPrincipals({ page: 1, size: 200 })
    ]);
    apiMilestones.value = milestones.items ?? [];
    apiTasks.value = tasks.items ?? [];
    apiPrincipals.value = principals.items ?? [];
  } catch {
    failed.value = true;
    apiMilestones.value = [];
    apiTasks.value = [];
  } finally {
    loading.value = false;
  }
}

let initialised = false;

onMounted(async () => {
  if (useMock) return;
  try {
    await scope.ensureLoaded();
  } catch {
    failed.value = true;
    return;
  }
  initialised = true;
  await loadGoals();
});

watch(projectId, () => {
  if (!useMock && initialised) void loadGoals();
});
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.goals.title')"
      :subtitle="t('workspace.pages.goals.subtitle')"
    />

    <div v-if="!useMock" class="ws-toolbar">
      <label class="ws-toolbar-label" for="goals-project">
        {{ t("workspace.pages.goals.project") }}
      </label>
      <select id="goals-project" v-model="selectedProjectId" class="ws-toolbar-select">
        <option value="">{{ t("workspace.pages.goals.pickProjectShort") }}</option>
        <option v-for="p in scope.projects" :key="p.id" :value="p.id">
          {{ p.name }}
        </option>
      </select>
    </div>
    <p v-if="!useMock && !projectId" class="ws-hint" data-test="goals-pick-project">
      {{ t("workspace.pages.goals.pickProject") }}
    </p>
    <p v-else-if="failed" class="ws-hint" data-test="goals-failed">
      {{ t("workspace.pages.goals.loadFailed") }}
    </p>
    <p
      v-else-if="!useMock && !loading && !pageGoals.length"
      class="ws-hint"
      data-test="goals-empty"
    >
      {{ t("workspace.pages.goals.empty") }}
    </p>

    <!-- 三视图切换（V1.2） -->
    <div class="ws-view-switch" role="tablist">
      <button
        v-for="item in views"
        :key="item.key"
        type="button"
        class="ws-view-tab ws-focusable"
        :class="{ 'is-active': view === item.key }"
        role="tab"
        :aria-selected="view === item.key"
        :data-test="item.test"
        @click="view = item.key"
      >
        {{ t(item.labelKey) }}
      </button>
    </div>

    <!-- 视图一：目标树 -->
    <template v-if="view === 'tree'">
      <section
        v-for="goal in pageGoals"
        :key="goal.id"
        class="ws-card ws-goal-card"
        data-test="goal-card"
      >
        <div class="ws-goal-head">
          <div>
            <h2 class="ws-goal-name">
              <span class="ws-goal-code">{{ goal.code }}</span>
              {{ goal.name }}
            </h2>
            <p class="ws-goal-desc">{{ goal.description }}</p>
          </div>
          <span class="ws-goal-progress-num">{{ goal.progress }}%</span>
        </div>
        <p class="ws-goal-basis">
          {{ t("workspace.pages.goals.subtitle") }} ·
          {{ goal.progressBasis }}
        </p>
        <div
          class="ws-goal-progress"
          role="progressbar"
          :aria-valuenow="goal.progress"
          aria-valuemin="0"
          aria-valuemax="100"
        >
          <div class="ws-goal-bar" :style="{ width: `${goal.progress}%` }" />
        </div>

        <h3 v-if="goal.milestones.length" class="ws-goal-sub">
          {{ t("workspace.pages.goals.milestones") }}
        </h3>
        <ul v-if="goal.milestones.length" class="ws-milestones">
          <li
            v-for="ms in goal.milestones"
            :key="ms.id"
            class="ws-milestone"
            data-test="milestone-item"
          >
            <span class="ws-badge" :class="msClass[ms.status]">
              {{ t(msLabel[ms.status]) }}
            </span>
            <span class="ws-milestone-name">{{ ms.name }}</span>
            <span class="ws-milestone-due">{{ ms.due }}</span>
          </li>
        </ul>

        <h3 class="ws-goal-sub">
          {{ t("workspace.pages.goalsExtra.approvedPlan") }}
        </h3>
        <ul class="ws-tasks">
          <li
            v-for="task in goal.tasks"
            :key="task.id"
            class="ws-task"
            data-test="task-item"
          >
            <span class="ws-badge" :class="taskClass[task.status]">
              {{ t(taskLabel[task.status]) }}
            </span>
            <span class="ws-task-name">{{ task.name }}</span>
            <span class="ws-task-owner">{{ task.owner }}</span>
          </li>
        </ul>

        <!-- V1.2：AI 提议的计划（草案）与已批准计划分离 -->
        <template v-if="goal.proposedTasks.length">
          <h3 class="ws-goal-sub is-draft">
            {{ t("workspace.pages.goalsExtra.proposed") }}
          </h3>
          <ul class="ws-tasks is-draft" data-test="proposed-tasks">
            <li
              v-for="task in goal.proposedTasks"
              :key="task.id"
              class="ws-task"
            >
              <span class="ws-badge task-proposed">
                {{ t("workspace.pages.goals.taskTodo") }}
              </span>
              <span class="ws-task-name">{{ task.name }}</span>
              <span class="ws-task-owner">{{ task.owner }}</span>
            </li>
          </ul>
        </template>

        <!-- 依赖关系块（原型对齐） -->
        <div
          v-if="goal.dependencies.length"
          class="ws-dep-block"
          data-test="goal-dependencies"
        >
          <div class="ws-dep-title">
            <GitBranch :size="13" aria-hidden="true" />
            {{ t("workspace.pages.goalsExtra.dependencies") }}
          </div>
          <div class="ws-dep-chain">
            <template v-for="(dep, i) in goal.dependencies" :key="dep">
              <span class="ws-dep-node">{{ dep }}</span>
              <span
                v-if="i < goal.dependencies.length - 1"
                class="ws-dep-arrow"
                aria-hidden="true"
                >→</span
              >
            </template>
          </div>
          <button
            v-if="goal.code === 'GOAL-001'"
            type="button"
            class="ws-dep-cta ws-focusable"
            data-test="view-blocked-decision"
            @click="router.push(`/workspace/proj-worldmap/decisions`)"
          >
            {{ t("workspace.pages.goalsExtra.viewBlockedDecision") }}
          </button>
        </div>
      </section>
    </template>

    <!-- 视图二：依赖图（链式节点） -->
    <template v-else-if="view === 'deps'">
      <p v-if="!hasDependencies" class="ws-hint" data-test="deps-empty">
        {{ t("workspace.pages.goalsExtra.noDependencies") }}
      </p>
      <section
        v-for="goal in pageGoals"
        :key="goal.id"
        class="ws-card ws-depview"
        data-test="dep-chain"
      >
        <h2 class="ws-goal-name">
          <span class="ws-goal-code">{{ goal.code }}</span>
          {{ goal.name }}
        </h2>
        <div class="ws-dep-chain is-large">
          <template v-for="(dep, i) in goal.dependencies" :key="dep">
            <span class="ws-dep-node is-large">{{ dep }}</span>
            <span
              v-if="i < goal.dependencies.length - 1"
              class="ws-dep-arrow is-large"
              aria-hidden="true"
              >→</span
            >
          </template>
        </div>
      </section>
    </template>

    <!-- 视图三：时间线（里程碑） -->
    <template v-else>
      <section class="ws-card ws-timeline" data-test="timeline">
        <div
          v-for="ms in timelineItems"
          :key="ms.id"
          class="ws-tl-item"
          data-test="timeline-item"
        >
          <span class="ws-badge" :class="msClass[ms.status]">
            {{ t(msLabel[ms.status]) }}
          </span>
          <span class="ws-tl-name">{{ ms.name }}</span>
          <span class="ws-tl-goal">{{ ms.goalCode }}</span>
          <span class="ws-tl-due">{{ ms.due }}</span>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 960px;
}

.ws-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-toolbar-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-toolbar-select {
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-surface);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
}

.ws-hint {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

/* 三视图切换（分段样式与顶栏模式开关一致） */
.ws-view-switch {
  display: inline-flex;
  align-self: flex-start;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow: hidden;
}

.ws-view-tab {
  height: 28px;
  padding: 0 12px;
  border: none;
  background: transparent;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.ws-view-tab.is-active {
  background: var(--ws-elevated);
  color: var(--ws-accent);
  font-weight: 600;
}

.ws-view-tab:not(.is-active):hover {
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
}

.ws-goal-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
}

.ws-goal-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.ws-goal-name {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-goal-code {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  margin-right: 6px;
  font-variant-numeric: tabular-nums;
}

.ws-goal-desc {
  margin: 4px 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-goal-basis {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-goal-progress-num {
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.ws-goal-progress {
  height: 4px;
  border-radius: 999px;
  background: var(--ws-elevated);
  overflow: hidden;
}

/* 进度是执行状态 → 语义蓝，不用 AI 紫（紫色克制） */
.ws-goal-bar {
  height: 100%;
  border-radius: 999px;
  background: var(--ws-color-info);
}

.ws-goal-sub {
  margin: 8px 0 0;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-goal-sub.is-draft {
  color: var(--ws-color-warning-text);
}

.ws-milestones,
.ws-tasks {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.ws-tasks.is-draft {
  opacity: 0.85;
}

.ws-milestone,
.ws-task {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: var(--ws-row-height);
  border-bottom: 1px solid var(--ws-border);
}

.ws-milestone:last-child,
.ws-task:last-child {
  border-bottom: none;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

.ws-badge.is-done {
  color: var(--ws-color-success-text);
}

.ws-badge.is-active {
  color: var(--ws-color-info-text);
}

.ws-badge.is-pending,
.ws-badge.task-todo {
  color: var(--ws-text-secondary);
}

.ws-badge.task-done {
  color: var(--ws-color-success-text);
}

.ws-badge.task-doing {
  color: var(--ws-color-info-text);
}

.ws-badge.task-blocked {
  color: var(--ws-color-danger-text);
}

.ws-badge.task-awaiting {
  color: var(--ws-color-warning-text);
}

.ws-badge.task-cancelled {
  color: var(--ws-text-secondary);
  text-decoration: line-through;
}

/* 草案任务：虚线描边区分（未批准不执行） */
.ws-badge.task-proposed {
  color: var(--ws-text-secondary);
  border-style: dashed;
}

.ws-milestone-name,
.ws-task-name {
  flex: 1;
  min-width: 0;
}

.ws-milestone-due,
.ws-task-owner {
  flex: none;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

/* 依赖关系块（原型对齐） */
.ws-dep-block {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border: 1px dashed var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ws-dep-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-dep-chain {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.ws-dep-node {
  padding: 3px 10px;
  border: 1px solid var(--ws-border);
  border-radius: 999px;
  background: var(--ws-surface);
  font-size: var(--ws-font-size-caption);
}

.ws-dep-arrow {
  color: var(--ws-text-secondary);
}

.ws-dep-cta {
  align-self: stretch;
  height: 32px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.ws-dep-cta:hover {
  background: var(--ws-surface);
}

/* 依赖图视图 */
.ws-depview {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}

.ws-dep-node.is-large {
  padding: 8px 16px;
  font-size: var(--ws-font-size-body);
}

.ws-dep-arrow.is-large {
  font-size: var(--ws-font-size-section);
}

/* 时间线视图 */
.ws-timeline {
  display: flex;
  flex-direction: column;
  padding: 4px 16px;
}

.ws-tl-item {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: var(--ws-row-height);
  border-bottom: 1px solid var(--ws-border);
}

.ws-tl-item:last-child {
  border-bottom: none;
}

.ws-tl-name {
  flex: 1;
  min-width: 0;
}

.ws-tl-goal,
.ws-tl-due {
  flex: none;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}
</style>
