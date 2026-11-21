<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ArrowLeft, Sparkles } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import { getTaskDetail, type TaskDetailView } from "@/api/task-plans";
import TasksFrame from "../TasksFrame.vue";
import TaskDetailTabs from "./TaskDetailTabs.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import TaskStatusChip from "../components/TaskStatusChip.vue";
import { shortId } from "../tasks-contract";
import { toDetailView, type DetailViewModel } from "./task-detail-adapter";

/**
 * 任务详情页根（S04-UI-03，接真）。
 *
 * 数据 = pm-api REST（GET /v1/pm/tasks/{task_id}）：任务本体 + 依据追溯
 * （需求 / 设计 / ADR / 实施基线）+ 上下游依赖 + 仓库绑定 + 验收标准
 * + 人工审批 + 审计历史，一次读完，页面不做 N+1。
 *
 * 诚实缺位：后端任务模型无「背景 / 预期结果 / 执行说明 / 上下文」独立字段，
 * 也无 AI 建议读端点——这些段渲染空态，不用演示文案填充。批准 / 退回是
 * 演示动作（无后端命令），已移除。
 */
const route = useRoute();
const router = useRouter();
const { t } = useI18n();

const taskId = computed(() => String(route.params.taskId ?? ""));
const raw = ref<TaskDetailView | null>(null);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!taskId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    raw.value = await getTaskDetail(taskId.value);
  } catch {
    loadError.value = true;
    raw.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(taskId, load);

const task = computed<DetailViewModel | null>(() =>
  raw.value ? toDetailView(raw.value) : null
);

const approval = computed(() => task.value?.approval ?? "none");

const breadcrumbs = computed(() => [
  { label: t("shell.app.side.item.taskManagement"), to: "/project/tasks" },
  { label: t("s04.tasks.tabs.backlog"), to: "/project/tasks/backlog" }
]);

function traceLabel(kind: string): string {
  switch (kind) {
    case "requirement":
      return t("s04.tasks.detail.traceRequirement");
    case "design":
      return t("s04.tasks.detail.traceDesign");
    case "adr":
      return t("s04.tasks.detail.traceAdr");
    default:
      return t("s04.tasks.detail.traceBaseline");
  }
}

function goBack() {
  void router.push("/project/tasks/backlog");
}
</script>

<template>
  <PageFrame
    :breadcrumbs="breadcrumbs"
    :title="task?.title ?? t('s04.tasks.detail.loading')"
    :description="
      task
        ? `${task.code} · ${t('s04.tasks.common.taskLabel')} ${shortId(task.id)}`
        : t('s04.tasks.detail.loading')
    "
  >
    <template #actions>
      <Button variant="secondary" size="sm" @click="goBack">
        <ArrowLeft :size="14" />
        {{ t("s04.tasks.detail.back") }}
      </Button>
    </template>

    <TasksFrame />
    <TaskDetailTabs v-if="taskId" :task-id="taskId" />

    <EmptyState
      v-if="loadError"
      :title="t('s04.tasks.detail.loadErrorTitle')"
      :description="t('s04.tasks.detail.loadErrorHint')"
    />

    <EmptyState
      v-else-if="!loading && !task"
      :title="t('s04.tasks.detail.notFoundTitle')"
      :description="t('s04.tasks.detail.notFoundHint')"
    />

    <template v-else-if="task">
      <section class="dt-header">
        <div class="dt-header__chips" data-testid="dt-chips">
          <TaskStatusChip :status="task.status" />
          <TaskPriorityChip :priority="task.priority" />
          <span class="dt-header__approval" :data-approval="approval">
            {{
              approval === "human_confirmed"
                ? t("s04.tasks.detail.approvalHuman")
                : approval === "pending"
                  ? t("s04.tasks.detail.approvalPending")
                  : t("s04.tasks.detail.approvalNone")
            }}
          </span>
        </div>
      </section>

      <div class="dt-layout">
        <div class="dt-main">
          <section class="dt-section" data-testid="dt-section-objective">
            <h2>{{ t("s04.tasks.detail.sectionObjective") }}</h2>
            <dl>
              <dt>{{ t("s04.tasks.detail.objective") }}</dt>
              <dd>
                {{ task.objective || t("s04.tasks.common.notAvailable") }}
              </dd>
              <dt>{{ t("s04.tasks.detail.scope") }}</dt>
              <dd>{{ task.scope || t("s04.tasks.common.notAvailable") }}</dd>
              <dt>{{ t("s04.tasks.detail.outOfScope") }}</dt>
              <dd>
                {{ task.outOfScope || t("s04.tasks.common.notAvailable") }}
              </dd>
            </dl>
          </section>

          <section class="dt-section" data-testid="dt-section-trace">
            <h2>{{ t("s04.tasks.detail.sectionTrace") }}</h2>
            <p v-if="task.traces.length === 0" class="dt-section__empty">
              {{ t("s04.tasks.detail.noTraces") }}
            </p>
            <ul v-else class="dt-traces">
              <li
                v-for="trace in task.traces"
                :key="`${trace.kind}-${trace.label}`"
                class="dt-trace"
              >
                <span class="dt-trace__kind">{{ traceLabel(trace.kind) }}</span>
                <span class="dt-trace__label">{{ trace.label }}</span>
                <span class="dt-trace__version"
                  >{{ t("s04.tasks.detail.traceVersion") }}
                  {{ trace.version }}</span
                >
                <span class="dt-trace__status">{{ trace.status }}</span>
              </li>
            </ul>
          </section>

          <section class="dt-section" data-testid="dt-section-deps">
            <h2>{{ t("s04.tasks.detail.sectionDeps") }}</h2>
            <p v-if="task.dependencies.length === 0" class="dt-section__empty">
              {{ t("s04.tasks.detail.noDependencies") }}
            </p>
            <ul v-else class="dt-deps">
              <li v-for="dep in task.dependencies" :key="dep.code">
                <span class="dt-deps__direction">
                  {{
                    dep.direction === "upstream"
                      ? t("s04.tasks.detail.dependsOn")
                      : t("s04.tasks.detail.blocks")
                  }}
                </span>
                <span class="dt-deps__code">{{ dep.code }}</span>
                <span class="dt-deps__title">{{ dep.title }}</span>
                <TaskStatusChip :status="dep.status" />
              </li>
            </ul>
          </section>

          <section class="dt-section" data-testid="dt-section-spec">
            <h2>{{ t("s04.tasks.detail.sectionSpec") }}</h2>
            <dl>
              <dt>{{ t("s04.tasks.detail.repo") }}</dt>
              <dd>{{ task.repo || t("s04.tasks.common.notAvailable") }}</dd>
              <dt>{{ t("s04.tasks.detail.module") }}</dt>
              <dd>{{ task.module || t("s04.tasks.common.notAvailable") }}</dd>
              <dt>{{ t("s04.tasks.detail.constraints") }}</dt>
              <dd>
                {{ task.constraints || t("s04.tasks.common.notAvailable") }}
              </dd>
            </dl>
          </section>

          <section class="dt-section" data-testid="dt-section-criteria">
            <h2>{{ t("s04.tasks.detail.sectionCriteria") }}</h2>
            <p v-if="task.criteria.length === 0" class="dt-section__empty">
              {{ t("s04.tasks.detail.noCriteria") }}
            </p>
            <table v-else class="dt-table">
              <thead>
                <tr>
                  <th>{{ t("s04.tasks.detail.criteriaId") }}</th>
                  <th>{{ t("s04.tasks.detail.criteriaDesc") }}</th>
                  <th>{{ t("s04.tasks.detail.criteriaMethod") }}</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="criterion in task.criteria" :key="criterion.id">
                  <td class="dt-table__mono">{{ criterion.id }}</td>
                  <td>{{ criterion.description }}</td>
                  <td>{{ criterion.method }}</td>
                </tr>
              </tbody>
            </table>
          </section>

          <section class="dt-section" data-testid="dt-section-delivery">
            <h2>{{ t("s04.tasks.detail.sectionDelivery") }}</h2>
            <p
              v-if="task.requiredDeliverables.length === 0"
              class="dt-section__empty"
            >
              {{ t("s04.tasks.detail.noDeliverables") }}
            </p>
            <div v-else class="dt-delivery">
              <div>
                <h3>{{ t("s04.tasks.detail.deliveryRequired") }}</h3>
                <ul>
                  <li v-for="item in task.requiredDeliverables" :key="item">
                    {{ item }}
                  </li>
                </ul>
              </div>
            </div>
          </section>

          <section class="dt-section" data-testid="dt-section-history">
            <h2>{{ t("s04.tasks.detail.sectionHistory") }}</h2>
            <p v-if="task.history.length === 0" class="dt-section__empty">
              {{ t("s04.tasks.detail.noHistory") }}
            </p>
            <ol v-else class="dt-history">
              <li
                v-for="event in task.history"
                :key="event.id"
                :class="`dt-history__item--${event.actor}`"
              >
                <span class="dt-history__badge">
                  {{ t("s04.tasks.detail.historyHuman") }}
                </span>
                <span class="dt-history__action">{{ event.action }}</span>
                <span class="dt-history__detail">{{ event.detail }}</span>
                <span class="dt-history__at">{{ event.at }}</span>
              </li>
            </ol>
          </section>
        </div>

        <aside class="dt-panel">
          <h2 class="dt-panel__title">
            <Sparkles :size="14" />
            {{ t("s04.tasks.detail.aiPanelTitle") }}
          </h2>
          <section class="dt-panel__section">
            <h3>{{ t("s04.tasks.detail.aiUnderstanding") }}</h3>
            <p>{{ task.objective || t("s04.tasks.common.notAvailable") }}</p>
          </section>
          <section class="dt-panel__section">
            <h3>{{ t("s04.tasks.detail.aiSuggestions") }}</h3>
            <EmptyState
              :title="t('s04.tasks.detail.aiSuggestionsUnavailable')"
              :description="t('s04.tasks.detail.aiSuggestionsHint')"
              size="sm"
            />
          </section>
        </aside>
      </div>
    </template>
  </PageFrame>
</template>

<style scoped>
.dt-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.dt-header__chips {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.dt-header__approval {
  padding: 0 var(--space-2);
  height: var(--size-control-sm);
  display: inline-flex;
  align-items: center;
  border-radius: var(--radius-full);
  border: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.dt-header__approval[data-approval="human_confirmed"] {
  border-color: var(--ws-color-success);
  color: var(--ws-color-success);
}

.dt-header__actions {
  display: flex;
  gap: var(--space-2);
}

.dt-layout {
  display: grid;
  grid-template-columns: minmax(0, 70fr) minmax(0, 30fr);
  gap: var(--space-5);
  align-items: start;
}

.dt-main {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.dt-section {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-4) var(--space-5);
}

.dt-section h2 {
  margin: 0 0 var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.dt-section dl {
  display: grid;
  grid-template-columns: minmax(var(--space-16), max-content) minmax(0, 1fr);
  gap: var(--space-2) var(--space-4);
  margin: 0;
}

.dt-section dt {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.dt-section dd {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.dt-traces {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.dt-trace {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.dt-trace__kind {
  min-width: var(--space-12);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.dt-trace__label {
  flex: 1;
  color: var(--ws-accent);
  font-size: var(--font-size-body);
}

.dt-trace__version,
.dt-trace__status {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.dt-deps {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.dt-deps li {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.dt-deps__direction {
  min-width: var(--space-12);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.dt-deps__code {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

.dt-deps__title {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.dt-table {
  width: 100%;
  border-collapse: collapse;
}

.dt-table th {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  text-align: left;
}

.dt-table td {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.dt-table__mono {
  font-family: var(--font-mono);
  color: var(--ws-text-secondary);
}

.dt-delivery {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.dt-delivery h3 {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.dt-delivery ul {
  margin: 0;
  padding-left: var(--space-4);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.dt-history {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.dt-history__item--ai,
.dt-history__item--human {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.dt-history__badge {
  min-width: var(--space-12);
  text-align: center;
  padding: 0 var(--space-2);
  height: var(--size-control-sm);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-full);
  font-size: var(--font-size-micro);
}

/* AI 动作与人工动作视觉区分：AI = Info 蓝描边（禁紫色），人工 = neutral */
.dt-history__item--ai .dt-history__badge {
  border: var(--border-1) solid var(--ws-color-info);
  color: var(--ws-color-info);
}

.dt-history__item--human .dt-history__badge {
  background: var(--ws-elevated);
  color: var(--ws-text-secondary);
}

.dt-history__action {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  white-space: nowrap;
}

.dt-history__detail {
  flex: 1;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.dt-history__at {
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
  font-variant-numeric: tabular-nums;
}

.dt-panel {
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

.dt-panel__title {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.dt-panel__section h3 {
  margin: 0 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.dt-panel__section p {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.dt-suggestions {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.dt-suggestion {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border: var(--border-1) dashed var(--ws-color-info);
  border-radius: var(--radius-sm);
}

.dt-suggestion p {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.dt-suggestion__actions {
  display: flex;
  gap: var(--space-2);
}

.dt-panel__applied {
  margin: 0;
  color: var(--ws-color-success);
  font-size: var(--font-size-caption);
}

@media (max-width: 1100px) {
  .dt-layout {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
