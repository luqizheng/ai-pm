<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ArrowLeft, Check, Sparkles, X } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import TaskStatusChip from "../components/TaskStatusChip.vue";
import { shortId } from "../tasks-contract";
import { taskDetailFixture, type TaskDetailFixture } from "./fixtures";

/**
 * 任务详情页根（S04-UI-03，mock 预览）。
 *
 * 后端无 GET 单任务读端点（执行说明/交付/验收标准仅 PATCH 可写、REST
 * 不可读），本页以 fixtures 演示 S04-UI-03 的信息模型：目标与范围 /
 * 来源追溯 / 依赖 / 执行说明 / 验收标准 / 交付要求 / 历史审计 + 右侧
 * AI Project Lead 建议。批准 / 退回等动作只改本页本地状态（演示审批
 * 流），不回写后端；路由仅在 VITE_USE_MOCK=1 注册真实页。
 */
const route = useRoute();
const router = useRouter();
const { t } = useI18n();

const task = taskDetailFixture;

const approval = ref<TaskDetailFixture["approval"]>(task.approval);
const appliedSuggestions = ref<Set<string>>(new Set());
const dismissedSuggestions = ref<Set<string>>(new Set());

const mockNote = t("s04.tasks.detail.mockNote");

const breadcrumbs = computed(() => [
  { label: t("shell.app.side.item.taskManagement"), to: "/project/tasks" },
  { label: t("s04.tasks.tabs.backlog"), to: "/project/tasks/backlog" }
]);

function applySuggestion(id: string) {
  appliedSuggestions.value = new Set([...appliedSuggestions.value, id]);
}

function dismissSuggestion(id: string) {
  dismissedSuggestions.value = new Set([...dismissedSuggestions.value, id]);
}

const visibleSuggestions = computed(() =>
  task.suggestions.filter(
    suggestion =>
      !dismissedSuggestions.value.has(suggestion.id) &&
      !appliedSuggestions.value.has(suggestion.id)
  )
);

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
    :title="task.title"
    :description="`${t('s04.tasks.common.taskLabel')} ${shortId(task.id)}`"
  >
    <template #actions>
      <Button variant="secondary" size="sm" @click="goBack">
        <ArrowLeft :size="14" />
        {{ t("s04.tasks.detail.back") }}
      </Button>
    </template>

    <TasksFrame />

    <Alert variant="info" :title="mockNote" />

    <section class="dt-header">
      <div class="dt-header__chips" data-testid="dt-chips">
        <TaskStatusChip :status="task.status" />
        <TaskPriorityChip :priority="task.priority" />
        <span class="dt-header__approval" :data-approval="approval">
          {{
            approval === "human_confirmed"
              ? t("s04.tasks.detail.approvalHuman")
              : t("s04.tasks.detail.approvalNone")
          }}
        </span>
      </div>
      <div class="dt-header__actions">
        <Button
          variant="primary"
          size="sm"
          data-testid="dt-approve"
          :disabled="approval === 'human_confirmed'"
          @click="approval = 'human_confirmed'"
        >
          <Check :size="13" />
          {{ t("s04.tasks.detail.approve") }}
        </Button>
        <Button
          variant="secondary"
          size="sm"
          data-testid="dt-return"
          :disabled="approval === 'none'"
          @click="approval = 'none'"
        >
          <X :size="13" />
          {{ t("s04.tasks.detail.unapprove") }}
        </Button>
      </div>
    </section>

    <div class="dt-layout">
      <div class="dt-main">
        <section class="dt-section" data-testid="dt-section-objective">
          <h2>{{ t("s04.tasks.detail.sectionObjective") }}</h2>
          <dl>
            <dt>{{ t("s04.tasks.detail.objective") }}</dt>
            <dd>{{ task.objective }}</dd>
            <dt>{{ t("s04.tasks.detail.background") }}</dt>
            <dd>{{ task.background }}</dd>
            <dt>{{ t("s04.tasks.detail.scope") }}</dt>
            <dd>{{ task.scope }}</dd>
            <dt>{{ t("s04.tasks.detail.outOfScope") }}</dt>
            <dd>{{ task.outOfScope }}</dd>
            <dt>{{ t("s04.tasks.detail.expected") }}</dt>
            <dd>{{ task.expected }}</dd>
          </dl>
        </section>

        <section class="dt-section" data-testid="dt-section-trace">
          <h2>{{ t("s04.tasks.detail.sectionTrace") }}</h2>
          <ul class="dt-traces">
            <li
              v-for="trace in task.traces"
              :key="trace.label"
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
          <ul class="dt-deps">
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
            <dd>{{ task.repo }}</dd>
            <dt>{{ t("s04.tasks.detail.module") }}</dt>
            <dd>{{ task.module }}</dd>
            <dt>{{ t("s04.tasks.detail.instructions") }}</dt>
            <dd>{{ task.instructions }}</dd>
            <dt>{{ t("s04.tasks.detail.constraints") }}</dt>
            <dd>{{ task.constraints }}</dd>
            <dt>{{ t("s04.tasks.detail.context") }}</dt>
            <dd>{{ task.context }}</dd>
          </dl>
        </section>

        <section class="dt-section" data-testid="dt-section-criteria">
          <h2>{{ t("s04.tasks.detail.sectionCriteria") }}</h2>
          <table class="dt-table">
            <thead>
              <tr>
                <th>{{ t("s04.tasks.detail.criteriaId") }}</th>
                <th>{{ t("s04.tasks.detail.criteriaDesc") }}</th>
                <th>{{ t("s04.tasks.detail.criteriaMethod") }}</th>
                <th>{{ t("s04.tasks.detail.criteriaEvidence") }}</th>
                <th>{{ t("s04.tasks.detail.criteriaMandatory") }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="criterion in task.criteria" :key="criterion.id">
                <td class="dt-table__mono">{{ criterion.id }}</td>
                <td>{{ criterion.description }}</td>
                <td>{{ criterion.method }}</td>
                <td>{{ criterion.evidence }}</td>
                <td>
                  {{
                    criterion.mandatory
                      ? t("s04.tasks.detail.criteriaMandatory")
                      : t("s04.tasks.detail.criteriaOptional")
                  }}
                </td>
              </tr>
            </tbody>
          </table>
        </section>

        <section class="dt-section" data-testid="dt-section-delivery">
          <h2>{{ t("s04.tasks.detail.sectionDelivery") }}</h2>
          <div class="dt-delivery">
            <div>
              <h3>{{ t("s04.tasks.detail.deliveryRequired") }}</h3>
              <ul>
                <li v-for="item in task.requiredDeliverables" :key="item">
                  {{ item }}
                </li>
              </ul>
            </div>
            <div>
              <h3>{{ t("s04.tasks.detail.deliveryOptional") }}</h3>
              <ul>
                <li v-for="item in task.optionalDeliverables" :key="item">
                  {{ item }}
                </li>
              </ul>
            </div>
          </div>
        </section>

        <section class="dt-section" data-testid="dt-section-history">
          <h2>{{ t("s04.tasks.detail.sectionHistory") }}</h2>
          <ol class="dt-history">
            <li
              v-for="event in task.history"
              :key="event.id"
              :class="`dt-history__item--${event.actor}`"
            >
              <span class="dt-history__badge">
                {{
                  event.actor === "ai"
                    ? t("s04.tasks.detail.historyAi")
                    : t("s04.tasks.detail.historyHuman")
                }}
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
          <p>{{ task.objective }}</p>
        </section>
        <section class="dt-panel__section">
          <h3>{{ t("s04.tasks.detail.aiMissing") }}</h3>
          <p>{{ t("s04.tasks.detail.context") }}：{{ task.context }}</p>
        </section>
        <section class="dt-panel__section">
          <h3>{{ t("s04.tasks.detail.aiSuggestions") }}</h3>
          <EmptyState
            v-if="visibleSuggestions.length === 0"
            :title="t('s04.tasks.overview.insightsEmpty')"
            size="sm"
          />
          <ul v-else class="dt-suggestions">
            <li
              v-for="suggestion in visibleSuggestions"
              :key="suggestion.id"
              class="dt-suggestion"
              :data-testid="`dt-suggestion-${suggestion.id}`"
            >
              <p>{{ suggestion.text }}</p>
              <div class="dt-suggestion__actions">
                <Button
                  variant="secondary"
                  size="sm"
                  @click="applySuggestion(suggestion.id)"
                >
                  {{ t("s04.tasks.detail.suggestionApply") }}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  @click="dismissSuggestion(suggestion.id)"
                >
                  {{ t("s04.tasks.detail.suggestionIgnore") }}
                </Button>
              </div>
            </li>
          </ul>
          <p
            v-if="appliedSuggestions.size > 0"
            class="dt-panel__applied"
            data-testid="dt-applied-count"
          >
            {{ t("s04.tasks.detail.suggestionApplied") }} ·
            {{ appliedSuggestions.size }}
          </p>
        </section>
      </aside>
    </div>
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
