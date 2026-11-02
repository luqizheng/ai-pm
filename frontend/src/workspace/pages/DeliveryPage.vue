<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import PageHeader from "./PageHeader.vue";
import {
  acceptanceDetails,
  acceptanceOverview,
  type AcceptanceDetailFixture
} from "../fixtures";
import {
  getAcceptanceOverview,
  listTaskDelivery,
  type AcceptanceBadge,
  type AcceptanceOverview,
  type AcceptanceTask,
  type TaskDeliveryRow
} from "@/api/acceptance";
import {
  decideReviewApproval,
  listCriterionVerdicts,
  listReviewFindings,
  listTaskReviewApprovals,
  type CriterionVerdict,
  type ReviewApproval,
  type ReviewFinding
} from "@/api/review";
import { listTaskRuns, type TaskRun } from "@/api/execution";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import { formatDate } from "@/api/format";

/**
 * 交付验收工作台（T-070 / docs/ai-pm-delivery-acceptance-workbench-v1.md §6）。
 *
 * - 验收动线：第一眼看到全部待验收交付（三指标 + 总表）→ 点进单任务核对证据
 *   → 通过 / 退回。徽标（badge）与动作区状态（review_status）全部消费后端
 *   字段，本组件**零资格推导**（Q2）；「待补充」任务不被过滤（Q1）。
 * - mock 分支与真后端分支在数据层分叉、模板唯一（同构）：fixtures 形状与
 *   API 响应完全一致。
 * - 写路径零新增：直接调 T-043 decision 端点；提交后原位刷新，不整页跳转。
 */

const { t } = useI18n();
const route = useRoute();
const scope = useScopeStore();

const useMock = import.meta.env.VITE_USE_MOCK === "1";

/* ------------------------------------------------------------ 顶部总表 */

const loading = ref(false);
const failed = ref(false);
const overview = ref<AcceptanceOverview | null>(null);
/** 三指标点数字过滤：all / eligible / blocked / missing。 */
const metricFilter = ref<"all" | AcceptanceBadge>("all");

const visibleTasks = computed(() => {
  const tasks = overview.value?.tasks ?? [];
  if (metricFilter.value === "all") return tasks;
  return tasks.filter(task => task.badge === metricFilter.value);
});

function badgeLabel(badge: AcceptanceBadge): string {
  return String(
    t(
      badge === "eligible"
        ? "workspace.pages.delivery.badgeEligible"
        : badge === "blocked"
          ? "workspace.pages.delivery.badgeBlocked"
          : "workspace.pages.delivery.badgeMissing"
    )
  );
}

const badgeClass: Record<AcceptanceBadge, string> = {
  eligible: "is-eligible",
  blocked: "is-blocked",
  missing: "is-missing"
};

function integrationLabel(status: string | null): string {
  if (status === "passed")
    return String(t("workspace.pages.delivery.integrationPassed"));
  if (status === "failed")
    return String(t("workspace.pages.delivery.integrationFailed"));
  return String(t("workspace.pages.delivery.integrationNotRun"));
}

function reviewStatusLabel(task: AcceptanceTask): string {
  switch (task.review_status) {
    case "awaiting_decision":
      return String(t("workspace.pages.delivery.reviewAwaiting"));
    case "running":
      return String(t("workspace.pages.delivery.reviewRunning"));
    case "decided":
      return String(t("workspace.pages.delivery.reviewDecided"));
    default:
      return String(t("workspace.pages.delivery.reviewNone"));
  }
}

/* ------------------------------------------------------------ 项目范围 */

const projectId = computed<string | undefined>(() => {
  if (scope.projectIdParam) return scope.projectIdParam;
  const param = String(route.params.projectId ?? "");
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      param
    )
  ) {
    return param;
  }
  return undefined;
});

const selectedProjectId = computed({
  get: () =>
    scope.currentProjectId === SCOPE_ALL ? "" : scope.currentProjectId,
  set: (value: string) => {
    scope.currentProjectId = value || SCOPE_ALL;
  }
});

/* ------------------------------------------------------------ 详情证据 */

const selectedTaskId = ref("");
const selectedTask = computed(() =>
  overview.value?.tasks.find(task => task.task_id === selectedTaskId.value)
);

const detailFailed = ref(false);
const deliveryRows = ref<TaskDeliveryRow[]>([]);
const findings = ref<ReviewFinding[]>([]);
const verdicts = ref<CriterionVerdict[]>([]);
const approvals = ref<ReviewApproval[]>([]);
const taskRuns = ref<TaskRun[]>([]);

const emptyDetail: AcceptanceDetailFixture = {
  delivery: [],
  findings: [],
  verdicts: [],
  approvals: [],
  runs: []
};

function applyDetail(detail: AcceptanceDetailFixture) {
  deliveryRows.value = detail.delivery;
  findings.value = detail.findings;
  verdicts.value = detail.verdicts;
  approvals.value = detail.approvals;
  taskRuns.value = detail.runs;
}

async function loadDetail(taskId: string) {
  applyDetail(emptyDetail);
  detailFailed.value = false;
  if (!taskId) return;
  if (useMock) {
    applyDetail(acceptanceDetails[taskId] ?? emptyDetail);
    return;
  }
  try {
    const [delivery, findingPage, verdictPage, approvalPage, runPage] =
      await Promise.all([
        listTaskDelivery(taskId),
        listReviewFindings(taskId),
        listCriterionVerdicts(taskId),
        listTaskReviewApprovals(taskId),
        listTaskRuns({ task_id: taskId, page: 1, size: 50 })
      ]);
    deliveryRows.value = delivery.items;
    findings.value = findingPage.items;
    verdicts.value = verdictPage.items;
    approvals.value = approvalPage.items;
    taskRuns.value = runPage.items;
  } catch {
    detailFailed.value = true;
  }
}

watch(selectedTaskId, taskId => {
  void loadDetail(taskId);
});

/* ------------------------------------------------------------ 总表加载 */

async function loadOverview() {
  if (useMock) {
    overview.value = acceptanceOverview;
    selectedTaskId.value = acceptanceOverview.tasks[0]?.task_id ?? "";
    return;
  }
  failed.value = false;
  if (!projectId.value) {
    overview.value = null;
    selectedTaskId.value = "";
    return;
  }
  loading.value = true;
  try {
    const next = await getAcceptanceOverview(projectId.value);
    overview.value = next;
    // 原位刷新时尽量保住当前选中；否则落到来。
    const stillThere = next.tasks.some(
      task => task.task_id === selectedTaskId.value
    );
    if (!stillThere) {
      selectedTaskId.value = next.tasks[0]?.task_id ?? "";
    }
  } catch {
    failed.value = true;
    overview.value = null;
  } finally {
    loading.value = false;
  }
}

let initialised = false;

onMounted(async () => {
  if (useMock) {
    await loadOverview();
    return;
  }
  try {
    await scope.ensureLoaded();
  } catch {
    failed.value = true;
    return;
  }
  initialised = true;
  await loadOverview();
});

watch(projectId, () => {
  if (!useMock && initialised) void loadOverview();
});

/* ------------------------------------------------------------ 验收动作 */

const reason = ref("");
const submitting = ref(false);
const actionError = ref("");
const decisionOutcome = ref("");

const canDecide = computed(
  () =>
    selectedTask.value?.review_status === "awaiting_decision" &&
    Boolean(selectedTask.value?.pending_approval_id)
);

function actionHint(task: AcceptanceTask): string {
  switch (task.review_status) {
    case "none":
      return String(t("workspace.pages.delivery.reviewNone"));
    case "running":
      return String(t("workspace.pages.delivery.reviewRunning"));
    case "decided":
      return String(t("workspace.pages.delivery.reviewDecided"));
    default:
      return "";
  }
}

function outcomeLabel(outcome: string): string {
  if (outcome === "completed")
    return String(t("workspace.pages.delivery.decisionCompleted"));
  if (outcome === "changes_requested")
    return String(t("workspace.pages.delivery.decisionChangesRequested"));
  if (outcome === "already_decided")
    return String(t("workspace.pages.delivery.decisionAlreadyDecided"));
  return outcome;
}

async function submitDecision(decision: "approved" | "rejected") {
  actionError.value = "";
  decisionOutcome.value = "";
  const task = selectedTask.value;
  if (!task?.pending_approval_id || submitting.value) return;
  const trimmed = reason.value.trim();
  if (decision === "rejected" && !trimmed) {
    actionError.value = String(
      t("workspace.pages.delivery.rejectReasonRequired")
    );
    return;
  }
  submitting.value = true;
  try {
    const outcome = await decideReviewApproval(task.pending_approval_id, {
      project_id: projectId.value ?? "",
      decision,
      reason: trimmed || undefined
    });
    decisionOutcome.value = outcome.outcome;
    reason.value = "";
    await Promise.all([loadOverview(), loadDetail(task.task_id)]);
  } catch {
    // anchor_drift / completion_gate_refused 等 409 由 http 层抛错，透出即可。
    actionError.value = String(t("workspace.pages.delivery.decisionFailed"));
  } finally {
    submitting.value = false;
  }
}

/* ------------------------------------------------------------ 渲染辅助 */

function deliveryStateLabel(row: TaskDeliveryRow): string {
  if (row.delivery_state === "delivered")
    return String(t("workspace.pages.delivery.deliveryStateDelivered"));
  if (row.delivery_state === "partial")
    return String(t("workspace.pages.delivery.deliveryStatePartial"));
  return String(t("workspace.pages.delivery.deliveryStateNeverSubmitted"));
}

function deliveryStateClass(row: TaskDeliveryRow): string {
  if (row.commit_sha) return "is-eligible";
  return "is-missing";
}

function testSummary(row: TaskDeliveryRow): string {
  const tests = row.test_evidence?.tests ?? [];
  if (!tests.length) return "";
  const passed = tests.filter(item => item.passed).length;
  return String(
    t("workspace.pages.delivery.testsSummary", { passed, total: tests.length })
  );
}

function verdictLabel(verdict: CriterionVerdict): string {
  if (!verdict.evaluated)
    return String(t("workspace.pages.delivery.verdictNotEvaluated"));
  switch (verdict.verdict) {
    case "passed":
      return String(t("workspace.pages.delivery.verdictPassed"));
    case "failed":
      return String(t("workspace.pages.delivery.verdictFailed"));
    case "not_evaluable":
      return String(t("workspace.pages.delivery.verdictNotEvaluable"));
    case "awaiting_manual":
      return String(t("workspace.pages.delivery.verdictAwaitingManual"));
    default:
      return verdict.verdict ?? "";
  }
}

function verdictClass(verdict: CriterionVerdict): string {
  if (!verdict.evaluated) return "is-missing";
  switch (verdict.verdict) {
    case "passed":
      return "is-eligible";
    case "failed":
      return "is-blocked";
    case "not_evaluable":
      return "is-missing";
    case "awaiting_manual":
      return "is-missing";
    default:
      return "is-missing";
  }
}

function severityClass(severity: string): string {
  if (severity === "blocker") return "is-blocked";
  if (severity === "warning") return "is-missing";
  return "is-muted";
}

function severityLabel(severity: string): string {
  if (severity === "blocker")
    return String(t("workspace.pages.delivery.severityBlocker"));
  if (severity === "warning")
    return String(t("workspace.pages.delivery.severityWarning"));
  return String(t("workspace.pages.delivery.severityInfo"));
}

function approvalStatusLabel(status: string): string {
  if (status === "pending")
    return String(t("workspace.pages.delivery.approvalPending"));
  if (status === "approved")
    return String(t("workspace.pages.delivery.approvalApproved"));
  return String(t("workspace.pages.delivery.approvalRejected"));
}

function approvalStatusClass(status: string): string {
  if (status === "approved") return "is-eligible";
  if (status === "rejected") return "is-blocked";
  return "is-missing";
}

function runKindLabel(run: TaskRun): string {
  return String(
    t(
      run.run_kind === "review"
        ? "workspace.pages.execution.runKindReview"
        : "workspace.pages.execution.runKindDelivery"
    )
  );
}

function runStatusLabel(status: string): string {
  const key = runStatusLabelKeys[status] ?? "";
  return key ? String(t(key)) : status;
}

const runStatusLabelKeys: Record<string, string> = {
  queued: "workspace.pages.execution.runQueued",
  claimed: "workspace.pages.execution.runQueued",
  running: "workspace.pages.execution.runRunning",
  succeeded: "workspace.pages.execution.runSucceeded",
  failed: "workspace.pages.execution.runFailed",
  timed_out: "workspace.pages.execution.runTimedOut",
  cancelled: "workspace.pages.execution.runCancelled"
};
</script>

<template>
  <div class="ws-page ws-delivery">
    <PageHeader
      :title="t('workspace.pages.delivery.title')"
      :subtitle="t('workspace.pages.delivery.subtitle')"
    />

    <!-- mock / 真后端在数据层分叉（loadOverview/loadDetail），模板唯一同构。 -->
    <div v-if="!useMock && !projectId" class="ws-toolbar-hint">
      <p class="ws-hint" data-test="delivery-pick-project">
        {{ t("workspace.pages.delivery.pickProject") }}
      </p>
    </div>
    <p
      v-else-if="!useMock && failed"
      class="ws-hint"
      data-test="delivery-failed"
    >
      {{ t("workspace.pages.delivery.loadFailed") }}
    </p>
    <p
      v-else-if="overview && !overview.tasks.length"
      class="ws-hint"
      data-test="delivery-empty"
    >
      {{ t("workspace.pages.delivery.empty") }}
    </p>
    <template v-else-if="overview">
      <div v-if="!useMock" class="ws-toolbar">
        <label class="ws-toolbar-label" for="delivery-project">
          {{ t("workspace.pages.delivery.project") }}
        </label>
        <select
          id="delivery-project"
          v-model="selectedProjectId"
          class="ws-toolbar-select"
        >
          <option value="">
            {{ t("workspace.pages.delivery.pickProjectShort") }}
          </option>
          <option v-for="p in scope.projects" :key="p.id" :value="p.id">
            {{ p.name }}
          </option>
        </select>
      </div>
      <div class="ws-metrics" data-test="delivery-metrics">
        <button
          type="button"
          class="ws-metric ws-focusable"
          :class="{ 'is-active': metricFilter === 'all' }"
          data-test="metric-all"
          @click="metricFilter = 'all'"
        >
          <span class="ws-metric-num">{{ overview.counts.pending }}</span>
          <span class="ws-metric-label">{{
            t("workspace.pages.delivery.metricPending")
          }}</span>
        </button>
        <button
          type="button"
          class="ws-metric ws-focusable is-eligible"
          :class="{ 'is-active': metricFilter === 'eligible' }"
          data-test="metric-eligible"
          @click="metricFilter = 'eligible'"
        >
          <span class="ws-metric-num">{{ overview.counts.eligible }}</span>
          <span class="ws-metric-label">{{
            t("workspace.pages.delivery.metricEligible")
          }}</span>
        </button>
        <button
          type="button"
          class="ws-metric ws-focusable is-blocked"
          :class="{ 'is-active': metricFilter === 'blocked' }"
          data-test="metric-blocked"
          @click="metricFilter = 'blocked'"
        >
          <span class="ws-metric-num">{{ overview.counts.blocked }}</span>
          <span class="ws-metric-label">{{
            t("workspace.pages.delivery.metricBlocked")
          }}</span>
        </button>
      </div>

      <div class="ws-delivery-body">
        <aside class="ws-card ws-task-list" data-test="delivery-task-list">
          <button
            v-for="task in visibleTasks"
            :key="task.task_id"
            type="button"
            class="ws-task ws-focusable"
            :class="{ 'is-selected': selectedTaskId === task.task_id }"
            data-test="delivery-task"
            @click="selectedTaskId = task.task_id"
          >
            <div class="ws-task-head">
              <span class="ws-badge" :class="badgeClass[task.badge]">
                {{ badgeLabel(task.badge) }}
              </span>
              <span class="ws-task-status">{{ task.status }}</span>
            </div>
            <div class="ws-task-title">{{ task.title }}</div>
            <div class="ws-task-summary">
              {{
                t("workspace.pages.delivery.expectedRepos", {
                  delivered: task.delivery_summary.delivered_repos,
                  expected: task.delivery_summary.expected_repos
                })
              }}
              <template v-if="task.blocker_count > 0">
                ·
                {{
                  t("workspace.pages.delivery.blockers", {
                    n: task.blocker_count
                  })
                }}
              </template>
            </div>
          </button>
        </aside>

        <article
          v-if="selectedTask"
          class="ws-card ws-delivery-detail"
          data-test="delivery-detail"
        >
          <!-- ① 验收资格横幅（后端字段，前端不推导） -->
          <div
            class="ws-banner"
            :class="badgeClass[selectedTask.badge]"
            data-test="delivery-banner"
          >
            <span class="ws-banner-title">
              {{
                selectedTask.badge === "eligible"
                  ? t("workspace.pages.delivery.eligibleBanner")
                  : t("workspace.pages.delivery.notEligibleBanner")
              }}
            </span>
            <span class="ws-banner-sub">
              {{ reviewStatusLabel(selectedTask) }}
              <template
                v-if="selectedTask.delivery_summary.missing_repos.length"
              >
                ·
                {{
                  t("workspace.pages.delivery.missingRepos", {
                    repos:
                      selectedTask.delivery_summary.missing_repos.join(", ")
                  })
                }}
              </template>
              ·
              {{
                integrationLabel(
                  selectedTask.delivery_summary.integration_status
                )
              }}
            </span>
          </div>

          <p
            v-if="detailFailed"
            class="ws-hint"
            data-test="delivery-detail-failed"
          >
            {{ t("workspace.pages.delivery.loadFailed") }}
          </p>
          <p
            v-else-if="
              !deliveryRows.length &&
              !findings.length &&
              !approvals.length &&
              !taskRuns.length
            "
            class="ws-hint"
            data-test="delivery-no-evidence"
          >
            {{ t("workspace.pages.delivery.noEvidence") }}
          </p>

          <template v-else>
            <!-- ② 多仓交付清单 -->
            <section
              v-if="deliveryRows.length"
              class="ws-section"
              data-test="delivery-manifest"
            >
              <h3 class="ws-section-title">
                {{ t("workspace.pages.delivery.sectionDelivery") }}
              </h3>
              <div
                v-for="row in deliveryRows"
                :key="row.repository_id"
                class="ws-repo-row"
                data-test="delivery-repo"
              >
                <span class="ws-repo-name">{{ row.repository_name }}</span>
                <span class="ws-badge" :class="deliveryStateClass(row)">
                  {{
                    row.commit_sha
                      ? deliveryStateLabel(row)
                      : t("workspace.pages.delivery.repoNotDelivered")
                  }}
                </span>
                <span v-if="row.commit_sha" class="ws-repo-meta ws-mono">
                  {{ row.branch }}@{{ row.commit_sha }} ·
                  {{ row.changed_files }}± · base {{ row.base_commit }}
                </span>
                <span v-if="testSummary(row)" class="ws-repo-meta">
                  {{ testSummary(row) }}
                </span>
              </div>
            </section>

            <!-- ③ 验收标准逐条裁决 -->
            <section
              v-if="verdicts.length"
              class="ws-section"
              data-test="delivery-verdicts"
            >
              <h3 class="ws-section-title">
                {{ t("workspace.pages.delivery.sectionVerdicts") }}
              </h3>
              <div
                v-for="verdict in verdicts"
                :key="verdict.criterion_id"
                class="ws-verdict-row"
                data-test="delivery-verdict"
              >
                <span class="ws-verdict-ordinal">{{ verdict.ordinal }}</span>
                <span class="ws-verdict-text">{{ verdict.text }}</span>
                <span class="ws-badge" :class="verdictClass(verdict)">
                  {{ verdictLabel(verdict) }}
                </span>
              </div>
            </section>

            <!-- ④ 验收问题（blocker 置顶由后端排序） -->
            <section
              v-if="findings.length"
              class="ws-section"
              data-test="delivery-findings"
            >
              <h3 class="ws-section-title">
                {{ t("workspace.pages.delivery.sectionFindings") }}
              </h3>
              <div
                v-for="finding in findings"
                :key="finding.id"
                class="ws-finding"
                data-test="delivery-finding"
              >
                <div class="ws-finding-head">
                  <span
                    class="ws-badge"
                    :class="severityClass(finding.severity)"
                  >
                    {{ severityLabel(finding.severity) }}
                  </span>
                  <span class="ws-finding-kind">{{ finding.kind }}</span>
                </div>
                <p class="ws-finding-message">{{ finding.message }}</p>
                <p v-if="finding.code_location" class="ws-finding-meta">
                  {{ t("workspace.pages.delivery.findingLocation") }}:
                  <span class="ws-mono">{{ finding.code_location }}</span>
                </p>
                <p v-if="finding.evidence_ref" class="ws-finding-meta">
                  {{ t("workspace.pages.delivery.findingEvidence") }}:
                  <span class="ws-mono">{{ finding.evidence_ref }}</span>
                </p>
              </div>
            </section>

            <!-- ⑤ Agent 执行情况（按 task_run，delivery/review 分轨） -->
            <section
              v-if="taskRuns.length"
              class="ws-section"
              data-test="delivery-runs"
            >
              <h3 class="ws-section-title">
                {{ t("workspace.pages.delivery.sectionRuns") }}
              </h3>
              <div
                v-for="run in taskRuns"
                :key="run.id"
                class="ws-runrow"
                data-test="delivery-run"
              >
                <span class="ws-badge is-muted">{{ runKindLabel(run) }}</span>
                <span class="ws-runrow-attempt">#{{ run.attempt_no }}</span>
                <span
                  class="ws-badge"
                  :class="
                    run.status === 'succeeded'
                      ? 'is-eligible'
                      : run.status === 'failed' || run.status === 'timed_out'
                        ? 'is-blocked'
                        : 'is-muted'
                  "
                >
                  {{ runStatusLabel(run.status) }}
                </span>
                <span class="ws-runrow-meta">
                  {{ formatDate(run.created_at) }}
                  <template v-if="run.executor_session">
                    · {{ run.executor_session }}
                  </template>
                </span>
              </div>
            </section>

            <!-- ⑥ 审批历史（含 pending，不可覆盖） -->
            <section
              v-if="approvals.length"
              class="ws-section"
              data-test="delivery-approvals"
            >
              <h3 class="ws-section-title">
                {{ t("workspace.pages.delivery.sectionApprovals") }}
              </h3>
              <div
                v-for="approval in approvals"
                :key="approval.id"
                class="ws-approval"
                data-test="delivery-approval"
              >
                <div class="ws-finding-head">
                  <span
                    class="ws-badge"
                    :class="approvalStatusClass(approval.status)"
                  >
                    {{ approvalStatusLabel(approval.status) }}
                  </span>
                  <span class="ws-finding-kind">{{ approval.kind }}</span>
                  <span class="ws-runrow-meta">
                    {{ formatDate(approval.created_at) }}
                  </span>
                </div>
                <p v-if="approval.decision_reason" class="ws-finding-meta">
                  {{ t("workspace.pages.delivery.approvalReason") }}:
                  {{ approval.decision_reason }}
                </p>
              </div>
            </section>
          </template>

          <!-- ⑦ 验收动作区（按钮可用性由后端字段驱动） -->
          <section class="ws-section ws-action" data-test="delivery-action">
            <h3 class="ws-section-title">
              {{ t("workspace.pages.delivery.sectionAction") }}
            </h3>
            <p
              v-if="!canDecide"
              class="ws-hint"
              data-test="delivery-action-hint"
            >
              {{ actionHint(selectedTask) }}
            </p>
            <p
              v-if="decisionOutcome"
              class="ws-hint is-outcome"
              data-test="delivery-decision-outcome"
            >
              {{ outcomeLabel(decisionOutcome) }}
            </p>
            <p
              v-if="actionError"
              class="ws-hint is-error"
              data-test="delivery-action-error"
            >
              {{ actionError }}
            </p>
            <template v-if="canDecide">
              <textarea
                v-model="reason"
                class="ws-reason"
                rows="2"
                :placeholder="t('workspace.pages.delivery.reasonPlaceholder')"
                data-test="delivery-reason"
              />
              <div class="ws-action-buttons">
                <button
                  type="button"
                  class="ws-btn is-approve ws-focusable"
                  :disabled="submitting"
                  data-test="delivery-approve"
                  @click="submitDecision('approved')"
                >
                  {{ t("workspace.pages.delivery.approve") }}
                </button>
                <button
                  type="button"
                  class="ws-btn is-reject ws-focusable"
                  :disabled="submitting"
                  data-test="delivery-reject"
                  @click="submitDecision('rejected')"
                >
                  {{ t("workspace.pages.delivery.reject") }}
                </button>
              </div>
            </template>
          </section>
        </article>
      </div>
    </template>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 1080px;
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

.ws-hint.is-error {
  color: var(--ws-color-danger-text);
}

.ws-hint.is-outcome {
  color: var(--ws-color-success-text);
}

.ws-mono {
  font-family: var(--font-mono, monospace);
}

/* 三指标条 */
.ws-metrics {
  display: flex;
  gap: 12px;
}

.ws-metric {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 10px 16px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-surface);
  color: var(--ws-text-primary);
  cursor: pointer;
  font: inherit;
}

.ws-metric.is-active {
  box-shadow: var(--ws-focus-ring);
}

.ws-metric-num {
  font-size: var(--ws-font-size-section);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.ws-metric-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-metric.is-eligible .ws-metric-num {
  color: var(--ws-color-success-text);
}

.ws-metric.is-blocked .ws-metric-num {
  color: var(--ws-color-danger-text);
}

/* 主从两栏 */
.ws-delivery-body {
  display: grid;
  grid-template-columns: 340px minmax(0, 1fr);
  gap: 12px;
  align-items: start;
}

.ws-task-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px;
}

.ws-task {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-primary);
  cursor: pointer;
  text-align: left;
  font: inherit;
}

.ws-task:hover {
  background: var(--ws-elevated);
}

.ws-task.is-selected {
  background: var(--ws-elevated);
  box-shadow: var(--ws-focus-ring);
}

.ws-task-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-task-status {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-task-title {
  font-weight: 600;
  font-size: var(--ws-font-size-body);
}

.ws-task-summary {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 徽标语义色：可验收 success / 待补充 warning / 存在阻塞 danger（§6.2） */
.ws-badge.is-eligible {
  color: var(--ws-color-success-text);
}

.ws-badge.is-missing {
  color: var(--ws-color-warning-text);
}

.ws-badge.is-blocked {
  color: var(--ws-color-danger-text);
}

.ws-badge.is-muted {
  color: var(--ws-text-secondary);
}

/* 详情卡 */
.ws-delivery-detail {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}

.ws-banner {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border: 1px solid currentcolor;
  border-radius: var(--ws-radius-card);
}

.ws-banner.is-eligible {
  color: var(--ws-color-success-text);
}

.ws-banner.is-missing {
  color: var(--ws-color-warning-text);
}

.ws-banner.is-blocked {
  color: var(--ws-color-danger-text);
}

.ws-banner-title {
  font-weight: 600;
  font-size: var(--ws-font-size-body);
}

.ws-banner-sub {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.ws-section-title {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-repo-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px 10px;
  padding: 8px 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  font-size: var(--ws-font-size-caption);
}

.ws-repo-name {
  font-weight: 600;
}

.ws-repo-meta {
  color: var(--ws-text-secondary);
  overflow-wrap: anywhere;
}

.ws-verdict-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  font-size: var(--ws-font-size-caption);
}

.ws-verdict-ordinal {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 1px solid var(--ws-border);
  font-variant-numeric: tabular-nums;
}

.ws-verdict-text {
  flex: 1;
  min-width: 0;
}

.ws-finding {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  font-size: var(--ws-font-size-caption);
}

.ws-finding-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-finding-kind {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono, monospace);
}

.ws-finding-message {
  margin: 0;
  font-size: var(--ws-font-size-body);
}

.ws-finding-meta {
  margin: 0;
  color: var(--ws-text-secondary);
  overflow-wrap: anywhere;
}

.ws-runrow {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  font-size: var(--ws-font-size-caption);
}

.ws-runrow-attempt {
  font-variant-numeric: tabular-nums;
}

.ws-runrow-meta {
  color: var(--ws-text-secondary);
}

.ws-approval {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  font-size: var(--ws-font-size-caption);
}

.ws-action {
  border-top: 1px solid var(--ws-border);
  padding-top: 10px;
}

.ws-reason {
  width: 100%;
  resize: vertical;
  padding: 8px 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-surface);
  color: var(--ws-text-primary);
  font: inherit;
  font-size: var(--ws-font-size-caption);
}

.ws-action-buttons {
  display: flex;
  gap: 10px;
}

.ws-btn {
  padding: 6px 16px;
  border-radius: var(--ws-radius-card);
  border: 1px solid var(--ws-border);
  background: var(--ws-surface);
  color: var(--ws-text-primary);
  cursor: pointer;
  font: inherit;
  font-size: var(--ws-font-size-caption);
}

.ws-btn:disabled {
  cursor: not-allowed;
  color: var(--ws-text-secondary);
}

.ws-btn.is-approve {
  color: var(--ws-color-success-text);
}

.ws-btn.is-reject {
  color: var(--ws-color-danger-text);
}
</style>
