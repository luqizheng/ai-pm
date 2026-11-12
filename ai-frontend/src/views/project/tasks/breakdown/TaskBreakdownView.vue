<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Select from "@/components/ui/select/Select.vue";
import { PageFrame } from "@/components/shell";
import {
  getTaskPlan,
  getTaskPlanProgress,
  listTaskPlans,
  type TaskPlanDetail,
  type TaskPlanEntry,
  type TaskPlanProgress
} from "@/api/task-plans";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import TaskStatusChip from "../components/TaskStatusChip.vue";
import { useDesignProject } from "../../design/use-design-project";
import {
  toBreakdownDrafts,
  toBreakdownMaterialized,
  toBreakdownPlan,
  toBreakdownProgress,
  type BreakdownPlanView
} from "./task-breakdown-adapter";

/**
 * 任务拆解页根（S04-UI-02，接真）。
 *
 * 数据 = pm-api REST：
 * - `GET /v1/pm/projects/{project_id}/task-plans` —— 项目下的任务计划；
 * - `GET /v1/pm/task-plans/{plan_id}` —— 计划详情（修订链 + 已正式化任务）；
 * - `GET /v1/pm/task-plans/{plan_id}/progress` —— 进度与阻塞清单。
 *
 * 任务拆解的**事实来源是计划当前修订的正文**（content.tasks[]）：每条草案
 * 带 key / 标题 / 描述 / 验收标准 / 依赖建议 / 仓库建议 / 优先级。
 *
 * 诚实缺位：拆解理由 / 来源追溯 / 缺失工作 / 潜在重叠 / 风险 / 建议子任务
 * 后端无字段，本页不渲染（不用演示文案填充）；本页只读——正式化（把草案
 * 转成正式任务）与审批裁决走命令端点，前端未接线，因此不提供「接受/拒绝」
 * 这类会让人误以为已落库的按钮。
 */
const router = useRouter();
const { t } = useI18n();
const { projectId, hasProject } = useDesignProject();

const plans = ref<TaskPlanEntry[]>([]);
const selectedPlanId = ref("");
const detail = ref<TaskPlanDetail | null>(null);
const progress = ref<TaskPlanProgress | null>(null);
const loading = ref(true);
const loadError = ref(false);

async function loadPlans() {
  if (!hasProject.value) {
    plans.value = [];
    detail.value = null;
    progress.value = null;
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    plans.value = await listTaskPlans(projectId.value);
    const nextId = plans.value.some(plan => plan.id === selectedPlanId.value)
      ? selectedPlanId.value
      : (plans.value[0]?.id ?? "");
    selectedPlanId.value = nextId;
    await loadPlan(nextId);
  } catch {
    loadError.value = true;
    plans.value = [];
    detail.value = null;
    progress.value = null;
  } finally {
    loading.value = false;
  }
}

async function loadPlan(planId: string) {
  if (!planId) {
    detail.value = null;
    progress.value = null;
    return;
  }
  try {
    const [planDetail, planProgress] = await Promise.all([
      getTaskPlan(planId),
      getTaskPlanProgress(planId)
    ]);
    detail.value = planDetail;
    progress.value = planProgress;
  } catch {
    detail.value = null;
    progress.value = null;
  }
}

onMounted(loadPlans);
watch(projectId, loadPlans);

function selectPlan(planId: string) {
  selectedPlanId.value = planId;
  void loadPlan(planId);
}

const planOptions = computed(() =>
  plans.value.map(plan => ({
    value: plan.id,
    label: `${plan.code} · ${plan.title}`
  }))
);

const plan = computed<BreakdownPlanView | null>(() => {
  const entry = plans.value.find(item => item.id === selectedPlanId.value);
  if (!entry) return null;
  return toBreakdownPlan(entry, detail.value);
});

const drafts = computed(() => toBreakdownDrafts(detail.value));
const materialized = computed(() => toBreakdownMaterialized(detail.value));
const progressView = computed(() => toBreakdownProgress(progress.value));

const selectedDraftKey = ref("");
const selectedDraft = computed(
  () => drafts.value.find(draft => draft.key === selectedDraftKey.value) ?? null
);

function planStatusLabel(status: string): string {
  const key = `s04.tasks.breakdown.planState${status
    .charAt(0)
    .toUpperCase()}${status.slice(1)}`;
  const translated = t(key);
  return translated === key ? status : translated;
}

function openTask(taskId: string) {
  void router.push(`/project/tasks/${taskId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('shell.app.side.item.taskManagement') }]"
    :title="t('s04.tasks.page.breakdownTitle')"
    :description="t('s04.tasks.page.breakdownDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="bd-refresh"
        @click="loadPlans"
      >
        <RefreshCw :size="14" />
        {{ t("s04.tasks.common.refresh") }}
      </Button>
    </template>

    <TasksFrame />

    <Alert
      v-if="loadError"
      variant="danger"
      :title="t('s04.tasks.common.errorTitle')"
    >
      {{ t("s04.tasks.breakdown.loadFailed") }}
    </Alert>

    <EmptyState
      v-else-if="!hasProject"
      :title="t('s04.tasks.common.noProjectTitle')"
      :description="t('s04.tasks.common.noProjectHint')"
    />

    <EmptyState
      v-else-if="!loading && plans.length === 0"
      :title="t('s04.tasks.breakdown.planEmpty')"
      :description="t('s04.tasks.breakdown.planEmptyHint')"
    />

    <template v-else>
      <Alert variant="info" :title="t('s04.tasks.breakdown.readOnlyNote')" />

      <section class="bd-baseline">
        <header class="bd-baseline__head">
          <h2 class="bd-baseline__title">
            {{ t("s04.tasks.breakdown.planTitle") }}
          </h2>
          <Select
            :model-value="selectedPlanId"
            :options="planOptions"
            :aria-label="t('s04.tasks.breakdown.planSelect')"
            data-testid="bd-plan-select"
            @update:model-value="value => selectPlan(String(value))"
          />
        </header>
        <p v-if="plan" class="bd-baseline__name">
          {{ plan.code }} · {{ plan.title }}
        </p>
        <dl v-if="plan" class="bd-baseline__facts">
          <div>
            <dt>{{ t("s04.tasks.breakdown.planStatus") }}</dt>
            <dd data-testid="bd-plan-status">
              {{ planStatusLabel(plan.status) }}
            </dd>
          </div>
          <div>
            <dt>{{ t("s04.tasks.breakdown.planBaseline") }}</dt>
            <dd>{{ plan.baselineId || t("s04.tasks.common.notAvailable") }}</dd>
          </div>
          <div>
            <dt>{{ t("s04.tasks.breakdown.planRevision") }}</dt>
            <dd>
              {{ plan.revisionVersion > 0 ? `v${plan.revisionVersion}` : "" }}
              {{ plan.revisionState }}
            </dd>
          </div>
          <div>
            <dt>{{ t("s04.tasks.breakdown.progressPlanned") }}</dt>
            <dd data-testid="bd-planned-count">
              {{ progressView.plannedTasks }}
            </dd>
          </div>
          <div>
            <dt>{{ t("s04.tasks.breakdown.progressTotal") }}</dt>
            <dd data-testid="bd-total-count">
              {{ progressView.totalTasks }}
            </dd>
          </div>
        </dl>
      </section>

      <div class="bd-workspace">
        <section class="bd-main">
          <header class="bd-main__head">
            <h2 class="bd-main__title">
              {{ t("s04.tasks.breakdown.treeTitle") }}
            </h2>
            <span
              v-if="drafts.length > 0"
              class="bd-summary"
              data-testid="bd-summary"
            >
              {{
                t("s04.tasks.breakdown.draftCount", { count: drafts.length })
              }}
            </span>
          </header>

          <EmptyState
            v-if="drafts.length === 0"
            :title="t('s04.tasks.breakdown.draftEmpty')"
            :description="t('s04.tasks.breakdown.draftEmptyHint')"
            size="sm"
          />

          <div v-else class="bd-feature">
            <div v-for="task in drafts" :key="task.key" class="bd-node">
              <div
                class="bd-row"
                :class="{ 'is-selected': selectedDraftKey === task.key }"
                :data-testid="`bd-row-${task.key}`"
                role="button"
                tabindex="0"
                @click="selectedDraftKey = task.key"
                @keydown.enter="selectedDraftKey = task.key"
              >
                <span class="bd-row__code">{{ task.key }}</span>
                <span class="bd-row__title">{{ task.title }}</span>
                <TaskPriorityChip :priority="task.priority" />
                <span class="bd-row__meta">{{
                  t("s04.tasks.breakdown.draftMeta", {
                    deps: task.dependsOn.length,
                    criteria: task.acceptanceCriteria.length
                  })
                }}</span>
                <span class="bd-row__state">
                  {{ t("s04.tasks.breakdown.draftBadge") }}
                </span>
              </div>
            </div>
          </div>

          <section class="bd-progress">
            <h2 class="bd-main__title">
              {{ t("s04.tasks.breakdown.progressTitle") }}
            </h2>
            <ul v-if="progressView.byStatus.length > 0" class="bd-status-list">
              <li
                v-for="row in progressView.byStatus"
                :key="row.status"
                :data-testid="`bd-status-${row.status}`"
              >
                <TaskStatusChip :status="row.status" />
                <span class="bd-status-list__count">{{ row.count }}</span>
              </li>
            </ul>
            <EmptyState
              v-else
              :title="t('s04.tasks.breakdown.progressEmpty')"
              size="sm"
            />
            <h3 class="bd-progress__subtitle">
              {{ t("s04.tasks.breakdown.progressBlocked") }}
            </h3>
            <EmptyState
              v-if="progressView.blocked.length === 0"
              :title="t('s04.tasks.breakdown.progressBlockedEmpty')"
              size="sm"
            />
            <ul v-else class="bd-status-list">
              <li
                v-for="item in progressView.blocked"
                :key="item.id"
                :data-testid="`bd-blocked-${item.code}`"
              >
                <span class="bd-row__code">{{ item.code }}</span>
                <span class="bd-row__title">{{ item.title }}</span>
                <span class="bd-row__meta">{{
                  t("s04.tasks.breakdown.openUpstreams", {
                    count: item.openUpstreams
                  })
                }}</span>
              </li>
            </ul>
          </section>

          <section class="bd-progress">
            <h2 class="bd-main__title">
              {{ t("s04.tasks.breakdown.materializedTitle") }}
            </h2>
            <EmptyState
              v-if="materialized.length === 0"
              :title="t('s04.tasks.breakdown.materializedEmpty')"
              size="sm"
            />
            <ul v-else class="bd-status-list">
              <li
                v-for="item in materialized"
                :key="item.taskId"
                :data-testid="`bd-task-${item.code}`"
              >
                <button
                  type="button"
                  class="bd-row__link"
                  @click="openTask(item.taskId)"
                >
                  <span class="bd-row__code">{{ item.code }}</span>
                  <span class="bd-row__title">{{ item.title }}</span>
                </button>
                <TaskStatusChip :status="item.status" />
              </li>
            </ul>
          </section>
        </section>

        <aside class="bd-panel">
          <h2 class="bd-panel__title">
            {{ t("s04.tasks.breakdown.panelTitle") }}
          </h2>
          <template v-if="selectedDraft">
            <section class="bd-panel__section">
              <h3>{{ t("s04.tasks.breakdown.panelDescription") }}</h3>
              <p>
                {{
                  selectedDraft.description ||
                  t("s04.tasks.common.notAvailable")
                }}
              </p>
            </section>
            <section class="bd-panel__section">
              <h3>{{ t("s04.tasks.breakdown.panelCriteria") }}</h3>
              <p
                v-if="selectedDraft.acceptanceCriteria.length === 0"
                class="bd-panel__empty"
              >
                {{ t("s04.tasks.common.notAvailable") }}
              </p>
              <ul v-else class="bd-panel__list">
                <li
                  v-for="criterion in selectedDraft.acceptanceCriteria"
                  :key="criterion"
                >
                  {{ criterion }}
                </li>
              </ul>
            </section>
            <section class="bd-panel__section">
              <h3>{{ t("s04.tasks.breakdown.panelDeps") }}</h3>
              <p
                v-if="selectedDraft.dependsOn.length === 0"
                class="bd-panel__empty"
              >
                {{ t("s04.tasks.common.notAvailable") }}
              </p>
              <ul v-else class="bd-panel__list">
                <li v-for="dep in selectedDraft.dependsOn" :key="dep">
                  {{ dep }}
                </li>
              </ul>
            </section>
            <section class="bd-panel__section">
              <h3>{{ t("s04.tasks.breakdown.panelRepos") }}</h3>
              <p
                v-if="selectedDraft.repositoryIds.length === 0"
                class="bd-panel__empty"
              >
                {{ t("s04.tasks.common.notAvailable") }}
              </p>
              <ul v-else class="bd-panel__list">
                <li v-for="repo in selectedDraft.repositoryIds" :key="repo">
                  {{ repo }}
                </li>
              </ul>
            </section>
            <section v-if="selectedDraft.change" class="bd-panel__section">
              <h3>{{ t("s04.tasks.breakdown.panelChange") }}</h3>
              <p>{{ selectedDraft.change }}</p>
            </section>
          </template>
          <EmptyState
            v-else
            :title="t('s04.tasks.breakdown.panelTitle')"
            :description="t('s04.tasks.breakdown.panelEmpty')"
            size="sm"
          />
        </aside>
      </div>
    </template>
  </PageFrame>
</template>

<style scoped>
.bd-baseline {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4) var(--space-5);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
}

.bd-baseline__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.bd-baseline__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.bd-baseline__facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.bd-baseline__facts dt {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.bd-baseline__facts dd {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  word-break: break-all;
}

.bd-baseline__name {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.bd-workspace {
  display: grid;
  grid-template-columns: minmax(0, 62fr) minmax(0, 38fr);
  gap: var(--space-5);
  align-items: start;
}

.bd-main {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.bd-main__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  margin-bottom: var(--space-3);
}

.bd-main__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.bd-summary {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.bd-node {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.bd-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-xs);
  cursor: pointer;
  min-height: var(--ws-row-height);
  border-left: calc(var(--space-1) / 2) dashed var(--ws-color-info);
  background: transparent;
}

.bd-row.is-selected {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  background: var(--ws-color-accent-surface);
}

.bd-row__code {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  min-width: var(--space-12);
}

.bd-row__title {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.bd-row__meta {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  white-space: nowrap;
}

.bd-row__state {
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  white-space: nowrap;
}

.bd-row__link {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1;
  border: none;
  background: none;
  padding: 0;
  text-align: left;
  cursor: pointer;
}

.bd-progress {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-4) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.bd-progress__subtitle {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.bd-status-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.bd-status-list li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.bd-status-list__count {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.bd-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  position: sticky;
  top: var(--space-4);
}

.bd-panel__title {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.bd-panel__section h3 {
  margin: 0 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.bd-panel__section p {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.bd-panel__section p.bd-panel__empty {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.bd-panel__list {
  margin: 0;
  padding-left: var(--space-4);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

@media (max-width: 1100px) {
  .bd-workspace {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
