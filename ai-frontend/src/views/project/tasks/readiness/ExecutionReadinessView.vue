<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { ClipboardCheck, ShieldCheck } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import {
  getExecutionReadiness,
  type ExecutionReadiness
} from "@/api/task-plans";
import TasksFrame from "../TasksFrame.vue";
import TaskStatusChip from "../components/TaskStatusChip.vue";
import { useDesignProject } from "../../design/use-design-project";
import {
  toReadinessView,
  type ReadinessSectionView,
  type ReadinessViewModel
} from "./execution-readiness-adapter";

/**
 * 执行准备页根（S04-UI-07，接真）。
 *
 * 数据 = pm-api REST（GET /v1/pm/projects/{project_id}/execution-readiness）：
 * 后端已按任务跑完 A–F 六段检查并给出三态汇总，前端只做展示聚合
 * （段级通过率、缺项清单、待处置问题）。
 *
 * 诚实缺位：
 * - **无写端点**——准备度问题没有「解决 / 豁免」命令（后端只有 mark-ready
 *   单点闸门），因此本页不提供会让人误以为已落库的处置按钮，只提供
 *   「打开任务」直达处置现场；
 * - S05 移交走命令端点，前端未接线 → 只呈现就绪事实，不提供移交按钮；
 * - AI 准备度分析后端无读端点 → 空态。
 */
const router = useRouter();
const { t } = useI18n();
const { projectId, hasProject } = useDesignProject();

const payload = ref<ExecutionReadiness | null>(null);
const loading = ref(true);
const loadError = ref(false);
const lastValidation = ref("");

async function load() {
  if (!hasProject.value) {
    payload.value = null;
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    payload.value = await getExecutionReadiness(projectId.value);
    lastValidation.value = new Date().toLocaleTimeString();
  } catch {
    loadError.value = true;
    payload.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const model = computed<ReadinessViewModel | null>(() =>
  payload.value ? toReadinessView(payload.value) : null
);

const segment = ref<"all" | "ready" | "needs_attention" | "blocked">("all");

const filteredTasks = computed(() => {
  const tasks = model.value?.tasks ?? [];
  if (segment.value === "all") return tasks;
  return tasks.filter(task => task.state === segment.value);
});

function stateLabel(state: string): string {
  if (state === "ready") return t("s04.tasks.readiness.stateReady");
  if (state === "blocked") return t("s04.tasks.readiness.stateBlocked");
  return t("s04.tasks.readiness.stateAttention");
}

function sectionLabel(section: string): string {
  const key = `s04.tasks.readiness.cat${section
    .charAt(0)
    .toUpperCase()}${section.slice(1)}`;
  const translated = t(key);
  return translated === key ? section : translated;
}

function sectionStateLabel(state: ReadinessSectionView["state"]): string {
  if (state === "pass") return t("s04.tasks.readiness.statePass");
  if (state === "block") return t("s04.tasks.readiness.stateBlock");
  return t("s04.tasks.readiness.stateWarn");
}

function severityLabel(severity: string): string {
  if (severity === "high") return t("s04.tasks.readiness.severityHigh");
  return t("s04.tasks.readiness.severityMedium");
}

function openTask(taskId: string) {
  void router.push(`/project/tasks/${taskId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('shell.app.side.item.taskManagement') }]"
    :title="t('s04.tasks.page.readinessTitle')"
    :description="t('s04.tasks.page.readinessDesc')"
  >
    <template #actions>
      <Button
        variant="primary"
        size="sm"
        :disabled="loading"
        data-testid="rd-validate"
        @click="load"
      >
        <ShieldCheck :size="14" />
        {{ t("s04.tasks.readiness.validate") }}
      </Button>
    </template>

    <TasksFrame />

    <Alert
      v-if="loadError"
      variant="danger"
      :title="t('s04.tasks.common.errorTitle')"
    >
      {{ t("s04.tasks.readiness.loadFailed") }}
    </Alert>

    <EmptyState
      v-else-if="!hasProject"
      :title="t('s04.tasks.common.noProjectTitle')"
      :description="t('s04.tasks.common.noProjectHint')"
    />

    <EmptyState
      v-else-if="!loading && model && model.tasks.length === 0"
      :title="t('s04.tasks.common.empty')"
      :description="t('s04.tasks.readiness.noTasksHint')"
    />

    <template v-else-if="model">
      <p v-if="lastValidation" class="rd-last" data-testid="rd-last">
        {{ t("s04.tasks.readiness.lastValidation", { time: lastValidation }) }}
      </p>

      <section class="rd-summary">
        <button
          v-for="seg in [
            {
              key: 'ready',
              count: model.readyCount,
              tone: 'ready',
              label: t('s04.tasks.readiness.stateReady')
            },
            {
              key: 'needs_attention',
              count: model.attentionCount,
              tone: 'warning',
              label: t('s04.tasks.readiness.stateAttention')
            },
            {
              key: 'blocked',
              count: model.blockedCount,
              tone: 'danger',
              label: t('s04.tasks.readiness.stateBlocked')
            }
          ]"
          :key="seg.key"
          type="button"
          class="rd-segment"
          :class="{ 'is-active': segment === seg.key }"
          :data-testid="`rd-seg-${seg.key}`"
          @click="segment = seg.key"
        >
          <span class="rd-segment__count">{{ seg.count }}</span>
          <span class="rd-segment__label">
            {{ seg.label }} ·
            {{ Math.round((seg.count / (model.totalCount || 1)) * 100) }}%
          </span>
        </button>
        <button
          type="button"
          class="rd-segment"
          :class="{ 'is-active': segment === 'all' }"
          data-testid="rd-seg-all"
          @click="segment = 'all'"
        >
          <span class="rd-segment__count">{{ model.totalCount }}</span>
          <span class="rd-segment__label">{{ t("s04.tasks.common.all") }}</span>
        </button>
      </section>

      <section class="rd-card">
        <header class="rd-card__head">
          <h2>
            <ClipboardCheck :size="14" />
            {{ t("s04.tasks.readiness.checklistTitle") }}
          </h2>
        </header>
        <div class="rd-checklist">
          <section
            v-for="section in model.sections"
            :key="section.section"
            class="rd-cat"
            :data-testid="`rd-section-${section.section}`"
          >
            <h3>{{ section.section }} · {{ sectionLabel(section.section) }}</h3>
            <p class="rd-cat__state">
              <span
                class="rd-item-state"
                :class="`rd-item-state--${section.state}`"
              >
                {{
                  section.state === "pass"
                    ? "✓"
                    : section.state === "block"
                      ? "⛔"
                      : "⚠"
                }}
              </span>
              <span class="rd-item-state-label">{{
                sectionStateLabel(section.state)
              }}</span>
              <span class="rd-cat__count">
                {{
                  t("s04.tasks.readiness.checkSummary", {
                    passed: section.passed,
                    total: section.total
                  })
                }}
              </span>
            </p>
            <EmptyState
              v-if="section.failures.length === 0"
              :title="t('s04.tasks.readiness.sectionClean')"
              size="sm"
            />
            <ul v-else class="rd-cat__failures">
              <li
                v-for="failure in section.failures"
                :key="failure.taskId"
                :data-testid="`rd-fail-${section.section}-${failure.code}`"
              >
                <button
                  type="button"
                  class="rd-row-link"
                  @click="openTask(failure.taskId)"
                >
                  <span class="rd-row-code">{{ failure.code }}</span>
                  {{ failure.title }}
                </button>
                <span class="rd-cat__issues">{{
                  failure.issues.join("；")
                }}</span>
              </li>
            </ul>
          </section>
        </div>
      </section>

      <section class="rd-card">
        <header class="rd-card__head">
          <h2>{{ t("s04.tasks.readiness.tableTitle") }}</h2>
          <span class="rd-card__hint"
            >{{ filteredTasks.length }} / {{ model.tasks.length }}</span
          >
        </header>
        <table class="rd-table">
          <thead>
            <tr>
              <th>{{ t("s04.tasks.common.taskLabel") }}</th>
              <th>{{ t("s04.tasks.common.statusLabel") }}</th>
              <th>{{ t("s04.tasks.readiness.colReadiness") }}</th>
              <th>{{ t("s04.tasks.readiness.colMissing") }}</th>
              <th>{{ t("s04.tasks.readiness.colChecks") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="task in filteredTasks"
              :key="task.taskId"
              :data-testid="`rd-row-${task.code}`"
            >
              <td>
                <button
                  type="button"
                  class="rd-row-link"
                  @click="openTask(task.taskId)"
                >
                  <span class="rd-row-code">{{ task.code }}</span>
                  {{ task.title }}
                </button>
              </td>
              <td><TaskStatusChip :status="task.status" /></td>
              <td>
                <span
                  class="rd-row-state"
                  :class="`rd-row-state--${task.state}`"
                  >{{ stateLabel(task.state) }}</span
                >
              </td>
              <td>{{ task.missing.join(", ") || "—" }}</td>
              <td>
                {{
                  t("s04.tasks.readiness.checkSummary", {
                    passed: task.passedChecks,
                    total: task.totalChecks
                  })
                }}
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      <section class="rd-card" data-testid="rd-issues">
        <header class="rd-card__head">
          <h2>{{ t("s04.tasks.readiness.issuesTitle") }}</h2>
          <span class="rd-card__hint"
            >{{ model.issues.length }}
            {{ t("s04.tasks.readiness.issueOpen") }}</span
          >
        </header>
        <EmptyState
          v-if="model.issues.length === 0"
          :title="t('s04.tasks.planning.validationClean')"
          size="sm"
        />
        <ul v-else class="rd-issue-list">
          <li
            v-for="issue in model.issues"
            :key="issue.id"
            class="rd-issue"
            :data-testid="`rd-issue-${issue.code}-${issue.key}`"
          >
            <div class="rd-issue__main">
              <span
                class="rd-issue__severity"
                :class="`rd-issue__severity--${issue.severity}`"
              >
                {{ severityLabel(issue.severity) }}
              </span>
              <span class="rd-issue__code">{{ issue.code }}</span>
              <span class="rd-issue__reason">{{ issue.reason }}</span>
            </div>
            <div class="rd-issue__actions">
              <Button
                variant="secondary"
                size="sm"
                @click="openTask(issue.taskId)"
              >
                {{ t("s04.tasks.readiness.openTask") }}
              </Button>
            </div>
          </li>
        </ul>
      </section>

      <section class="rd-card" data-testid="rd-recs">
        <header class="rd-card__head">
          <h2>{{ t("s04.tasks.readiness.aiTitle") }}</h2>
        </header>
        <EmptyState
          :title="t('s04.tasks.readiness.aiUnavailable')"
          :description="t('s04.tasks.readiness.aiUnavailableHint')"
          size="sm"
        />
      </section>

      <section class="rd-card rd-handoff" data-testid="rd-handoff">
        <header class="rd-card__head">
          <h2>{{ t("s04.tasks.readiness.handoffTitle") }}</h2>
        </header>
        <div class="rd-handoff__facts">
          <span>{{
            t("s04.tasks.readiness.handoffIncluded", {
              count: model.totalCount
            })
          }}</span>
          <span>{{
            t("s04.tasks.readiness.handoffExcluded", {
              count: model.attentionCount + model.blockedCount
            })
          }}</span>
          <span>{{
            t("s04.tasks.readiness.handoffIssues", {
              count: model.issues.length
            })
          }}</span>
        </div>
        <p
          v-if="model.status !== 'ready'"
          class="rd-handoff__hint"
          data-testid="rd-handoff-blocked"
        >
          {{ t("s04.tasks.readiness.handoffBlockedHint") }}
        </p>
        <p class="rd-handoff__hint" data-testid="rd-handoff-unavailable">
          {{ t("s04.tasks.readiness.handoffUnavailable") }}
        </p>
      </section>
    </template>
  </PageFrame>
</template>

<style scoped>
.rd-last {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.rd-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
}

.rd-segment {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  text-align: left;
  cursor: pointer;
}

.rd-segment.is-active {
  border-color: var(--ws-accent);
}

.rd-segment__count {
  color: var(--ws-text-primary);
  font-size: var(--font-size-stat);
  font-variant-numeric: tabular-nums;
}

.rd-segment__label {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.rd-card {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-4) var(--space-5);
}

.rd-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-3);
}

.rd-card__head h2 {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.rd-card__hint {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.rd-checklist {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

.rd-cat h3 {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.rd-cat__state {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0 0 var(--space-2);
}

.rd-cat__count {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  font-variant-numeric: tabular-nums;
}

.rd-cat__failures {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rd-cat__failures li {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.rd-cat__issues {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.rd-item-state {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--space-5);
  height: var(--space-5);
  border-radius: var(--radius-full);
  font-size: var(--font-size-micro);
  font-style: normal;
}

.rd-item-state--pass {
  border: var(--border-1) solid var(--ws-color-success);
  color: var(--ws-color-success);
}

.rd-item-state--warn {
  border: var(--border-1) solid var(--ws-color-warning);
  color: var(--ws-color-warning);
}

.rd-item-state--block {
  border: var(--border-1) solid var(--ws-color-danger);
  color: var(--ws-color-danger);
}

.rd-item-state--na {
  border: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-disabled);
}

.rd-item-state-label {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.rd-table {
  width: 100%;
  border-collapse: collapse;
}

.rd-table th {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  text-align: left;
}

.rd-table td {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.rd-row-link {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-3);
  border: none;
  background: none;
  padding: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  cursor: pointer;
  text-align: left;
}

.rd-row-link:hover {
  color: var(--ws-accent);
}

.rd-row-code {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

.rd-row-state {
  font-size: var(--font-size-caption);
}

.rd-row-state--ready {
  color: var(--ws-color-status-ready);
}

.rd-row-state--needs_attention {
  color: var(--ws-color-warning);
}

.rd-row-state--blocked {
  color: var(--ws-color-danger);
}

.rd-issue-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rd-issue {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.rd-issue__main {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.rd-issue__severity {
  padding: 0 var(--space-2);
  height: var(--size-control-sm);
  display: inline-flex;
  align-items: center;
  border-radius: var(--radius-full);
  border: var(--border-1) solid transparent;
  font-size: var(--font-size-micro);
}

.rd-issue__severity--high {
  border-color: var(--ws-color-danger);
  color: var(--ws-color-danger);
}

.rd-issue__severity--medium {
  border-color: var(--ws-color-warning);
  color: var(--ws-color-warning);
}

.rd-issue__code {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

.rd-issue__reason {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.rd-issue__actions {
  display: flex;
  gap: var(--space-2);
}

.rd-handoff__facts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-4);
  margin-bottom: var(--space-3);
}

.rd-handoff__facts span {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.rd-handoff__hint {
  margin: 0 0 var(--space-3);
  color: var(--ws-color-warning);
  font-size: var(--font-size-caption);
}

@media (max-width: 1100px) {
  .rd-summary,
  .rd-checklist {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
