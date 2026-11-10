<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { ClipboardCheck, ShieldCheck, Sparkles } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import Dialog from "@/components/ui/dialog/Dialog.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Input from "@/components/ui/input/Input.vue";
import { PageFrame } from "@/components/shell";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import {
  readinessChecklistFixture,
  readinessIssuesFixture,
  readinessPlan,
  readinessRecsFixture,
  readinessTasksFixture,
  type IssueState,
  type ReadinessState
} from "./fixtures";

/**
 * 执行准备页根（S04-UI-07，mock 预览）。
 *
 * 准备度聚合与 S05 移交端点未落地（后端仅 mark-ready 单点闸门），
 * 本页以 fixtures 演示质量门：三类准备度汇总（可点过滤）、A–F 检查表
 * （状态随问题联动）、任务准备度表、问题处置（**解决或留痕豁免**——
 * 豁免必须填理由，仅当条件确实无法满足）、AI 分析、S05 移交（存在
 * 未决问题或审批未完即禁用；确认对话框列明纳入/排除/版本）。
 * 本页不启动 Agent、不执行代码；动作只改本页会话状态。
 */
const router = useRouter();
const { t } = useI18n();

const plan = readinessPlan;
const issues = ref(
  readinessIssuesFixture().map(issue => ({
    ...issue,
    state: "open" as IssueState,
    waiverReason: ""
  }))
);
const tasks = ref(readinessTasksFixture());
const recs = ref(readinessRecsFixture());
const reviewedRecs = ref<Set<string>>(new Set());

const validateRunning = ref(false);
const lastValidation = ref("");

const segment = ref<"all" | ReadinessState>("all");
const handoffDelivered = ref(false);

const issueStateOf = (id: string): IssueState =>
  issues.value.find(issue => issue.id === id)?.state ?? "open";

function itemState(item: {
  baseState: "pass" | "na";
  issueIds: string[];
}): "pass" | "na" | "warn" | "block" {
  const open = item.issueIds.filter(id => issueStateOf(id) === "open");
  if (open.length === 0) return item.baseState;
  const anyBlocking = issues.value.some(
    issue => open.includes(issue.id) && issue.blocking
  );
  return anyBlocking ? "block" : "warn";
}

const checklist = computed(() =>
  readinessChecklistFixture.map(item => ({ ...item, state: itemState(item) }))
);

const openIssues = computed(() =>
  issues.value.filter(issue => issue.state === "open")
);

const handoffEnabled = computed(
  () => openIssues.value.length === 0 && !handoffDelivered.value
);

// ─── 校验 ──────────────────────────────────────────────────────────────

async function validate() {
  if (validateRunning.value) return;
  validateRunning.value = true;
  // 演示校验耗时；真实实现为聚合端点
  await new Promise(resolve => setTimeout(resolve, 300));
  lastValidation.value = new Date().toLocaleTimeString();
  validateRunning.value = false;
}

// ─── 问题处置 ──────────────────────────────────────────────────────────

const waiveTarget = ref<string>("");
const waiveReason = ref("");
const waiveError = ref("");

const waiveOpen = computed(() => waiveTarget.value !== "");

function openWaive(issueId: string) {
  waiveTarget.value = issueId;
  waiveReason.value = "";
  waiveError.value = "";
}

function confirmWaive() {
  if (!waiveReason.value.trim()) {
    waiveError.value = t("s04.tasks.readiness.waivePlaceholder");
    return;
  }
  issues.value = issues.value.map(issue =>
    issue.id === waiveTarget.value
      ? { ...issue, state: "waived", waiverReason: waiveReason.value.trim() }
      : issue
  );
  waiveTarget.value = "";
}

function resolveIssue(issueId: string) {
  // 演示语义：处置动作已在任务详情完成，此处落账
  issues.value = issues.value.map(issue =>
    issue.id === issueId ? { ...issue, state: "resolved" } : issue
  );
}

function openTask(code: string) {
  void router.push(`/project/tasks/${code}`);
}

// ─── 表过滤 ────────────────────────────────────────────────────────────

const filteredTasks = computed(() =>
  segment.value === "all"
    ? tasks.value
    : tasks.value.filter(task => task.state === segment.value)
);

function stateLabel(state: ReadinessState): string {
  if (state === "ready") return t("s04.tasks.readiness.stateReady");
  if (state === "attention") return t("s04.tasks.readiness.stateAttention");
  return t("s04.tasks.readiness.stateBlocked");
}

// ─── AI 分析 ───────────────────────────────────────────────────────────

const visibleRecs = computed(() =>
  recs.value.filter(rec => !reviewedRecs.value.has(rec.id))
);

function reviewRec(id: string) {
  reviewedRecs.value = new Set([...reviewedRecs.value, id]);
}

// ─── S05 移交 ─────────────────────────────────────────────────────────

const handoffConfirmOpen = ref(false);

const handoffDescription = computed(() =>
  t("s04.tasks.readiness.handoffConfirmDesc", {
    version: plan.version,
    included: plan.included,
    excluded: plan.excluded,
    issues: openIssues.value.length
  })
);

function confirmHandoff() {
  handoffConfirmOpen.value = false;
  handoffDelivered.value = true;
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
        :disabled="validateRunning"
        data-testid="rd-validate"
        @click="validate"
      >
        <ShieldCheck :size="14" />
        {{
          validateRunning
            ? t("s04.tasks.readiness.validateRunning")
            : t("s04.tasks.readiness.validate")
        }}
      </Button>
    </template>

    <TasksFrame />

    <Alert variant="info" :title="t('s04.tasks.readiness.mockNote')" />

    <p v-if="lastValidation" class="rd-last" data-testid="rd-last">
      {{ t("s04.tasks.readiness.lastValidation", { time: lastValidation }) }}
    </p>

    <section class="rd-summary">
      <button
        v-for="seg in [
          { key: 'ready', count: plan.ready, tone: 'ready' },
          { key: 'attention', count: plan.attention, tone: 'warning' },
          { key: 'blocked', count: plan.blocked, tone: 'danger' }
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
          {{
            t(
              `s04.tasks.readiness.state${seg.key.charAt(0).toUpperCase()}${seg.key.slice(1)}`
            )
          }}
          · {{ Math.round((seg.count / (plan.included || 1)) * 100) }}%
        </span>
      </button>
      <button
        type="button"
        class="rd-segment"
        :class="{ 'is-active': segment === 'all' }"
        data-testid="rd-seg-all"
        @click="segment = 'all'"
      >
        <span class="rd-segment__count">{{ plan.included }}</span>
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
          v-for="cat in [
            'catDefinition',
            'catTrace',
            'catDependency',
            'catPreparation',
            'catAcceptance',
            'catApproval'
          ]"
          :key="cat"
          class="rd-cat"
        >
          <h3>{{ t(`s04.tasks.readiness.${cat}`) }}</h3>
          <ul>
            <li
              v-for="item in checklist.filter(entry => entry.category === cat)"
              :key="item.key"
              :data-testid="`rd-item-${item.key}`"
            >
              <i class="rd-item-state" :class="`rd-item-state--${item.state}`">
                {{
                  item.state === "pass"
                    ? "✓"
                    : item.state === "block"
                      ? "⛔"
                      : item.state === "warn"
                        ? "⚠"
                        : "—"
                }}
              </i>
              <span class="rd-item-text">{{
                t(`s04.tasks.readiness.${item.key}`)
              }}</span>
              <span class="rd-item-state-label">{{
                t(
                  `s04.tasks.readiness.state${item.state.charAt(0).toUpperCase()}${item.state.slice(1)}`
                )
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
          >{{ filteredTasks.length }} / {{ tasks.length }}</span
        >
      </header>
      <table class="rd-table">
        <thead>
          <tr>
            <th>{{ t("s04.tasks.common.taskLabel") }}</th>
            <th>{{ t("s04.tasks.common.priorityLabel") }}</th>
            <th>{{ t("s04.tasks.readiness.colReadiness") }}</th>
            <th>{{ t("s04.tasks.readiness.colMissing") }}</th>
            <th>{{ t("s04.tasks.backlog.colMilestone") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="task in filteredTasks"
            :key="task.code"
            :data-testid="`rd-row-${task.code}`"
          >
            <td>
              <button
                type="button"
                class="rd-row-link"
                @click="openTask(task.code)"
              >
                <span class="rd-row-code">{{ task.code }}</span>
                {{ task.title }}
              </button>
            </td>
            <td><TaskPriorityChip :priority="task.priority" /></td>
            <td>
              <span
                class="rd-row-state"
                :class="`rd-row-state--${task.state}`"
                >{{ stateLabel(task.state) }}</span
              >
            </td>
            <td>{{ task.missing.join(", ") || "—" }}</td>
            <td>{{ task.repo ?? "—" }}</td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="rd-card" data-testid="rd-issues">
      <header class="rd-card__head">
        <h2>{{ t("s04.tasks.readiness.issuesTitle") }}</h2>
        <span class="rd-card__hint"
          >{{ openIssues.length }}
          {{ t("s04.tasks.readiness.issueOpen") }}</span
        >
      </header>
      <EmptyState
        v-if="issues.length === 0"
        :title="t('s04.tasks.planning.validationClean')"
        size="sm"
      />
      <ul v-else class="rd-issue-list">
        <li
          v-for="issue in issues"
          :key="issue.id"
          class="rd-issue"
          :class="{ 'is-closed': issue.state !== 'open' }"
          :data-testid="`rd-issue-${issue.id}`"
          :data-state="issue.state"
        >
          <div class="rd-issue__main">
            <span
              class="rd-issue__severity"
              :class="`rd-issue__severity--${issue.severity}`"
            >
              {{
                t(
                  `s04.tasks.readiness.severity${issue.severity.charAt(0).toUpperCase()}${issue.severity.slice(1)}`
                )
              }}
            </span>
            <span class="rd-issue__code">{{ issue.code }}</span>
            <span class="rd-issue__reason">{{ issue.reason }}</span>
            <span class="rd-issue__state">
              {{
                issue.state === "open"
                  ? t("s04.tasks.readiness.issueOpen")
                  : issue.state === "resolved"
                    ? t("s04.tasks.readiness.issueResolved")
                    : t("s04.tasks.readiness.issueWaived")
              }}
              <template v-if="issue.waiverReason"
                >（{{ issue.waiverReason }}）</template
              >
            </span>
          </div>
          <p class="rd-issue__resolution">
            {{ t("s04.tasks.readiness.resolution") }}：{{ issue.resolution }}
          </p>
          <div v-if="issue.state === 'open'" class="rd-issue__actions">
            <Button variant="secondary" size="sm" @click="openTask(issue.code)">
              {{ t("s04.tasks.readiness.openTask") }}
            </Button>
            <Button
              variant="primary"
              size="sm"
              :data-testid="`rd-resolve-${issue.id}`"
              @click="resolveIssue(issue.id)"
            >
              {{ t("s04.tasks.readiness.markResolved") }}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              :data-testid="`rd-waive-${issue.id}`"
              @click="openWaive(issue.id)"
            >
              {{ t("s04.tasks.readiness.waive") }}
            </Button>
          </div>
        </li>
      </ul>
    </section>

    <section class="rd-card" data-testid="rd-recs">
      <header class="rd-card__head">
        <h2>
          <Sparkles :size="14" />
          {{ t("s04.tasks.readiness.aiTitle") }}
        </h2>
      </header>
      <EmptyState
        v-if="visibleRecs.length === 0"
        :title="t('s04.tasks.overview.insightsEmpty')"
        size="sm"
      />
      <ul v-else class="rd-recs">
        <li
          v-for="rec in visibleRecs"
          :key="rec.id"
          :data-testid="`rd-rec-${rec.id}`"
        >
          <p>{{ rec.text }}</p>
          <Button variant="secondary" size="sm" @click="reviewRec(rec.id)">
            {{ t("s04.tasks.overview.attentionGo") }}
          </Button>
        </li>
      </ul>
    </section>

    <section class="rd-card rd-handoff" data-testid="rd-handoff">
      <header class="rd-card__head">
        <h2>{{ t("s04.tasks.readiness.handoffTitle") }}</h2>
        <span class="rd-card__hint">{{
          t("s04.tasks.readiness.handoffVersion", { version: plan.version })
        }}</span>
      </header>
      <Alert
        v-if="handoffDelivered"
        variant="success"
        :title="t('s04.tasks.readiness.handoffDone', { version: plan.version })"
        data-testid="rd-handoff-done"
      >
        {{ t("s04.tasks.readiness.handoffDoneDesc") }}
      </Alert>
      <template v-else>
        <div class="rd-handoff__facts">
          <span>{{
            t("s04.tasks.readiness.handoffIncluded", { count: plan.included })
          }}</span>
          <span>{{
            t("s04.tasks.readiness.handoffExcluded", { count: plan.excluded })
          }}</span>
          <span>{{
            t("s04.tasks.readiness.handoffIssues", { count: openIssues.length })
          }}</span>
        </div>
        <p
          v-if="!handoffEnabled"
          class="rd-handoff__hint"
          data-testid="rd-handoff-blocked"
        >
          {{ t("s04.tasks.readiness.handoffBlockedHint") }}
        </p>
        <Button
          variant="primary"
          size="sm"
          :disabled="!handoffEnabled"
          data-testid="rd-handoff-btn"
          @click="handoffConfirmOpen = true"
        >
          {{ t("s04.tasks.readiness.handoffAction") }}
        </Button>
      </template>
    </section>

    <Dialog
      :open="waiveOpen"
      :title="t('s04.tasks.readiness.waiveTitle')"
      size="md"
      @update:open="waiveTarget = ''"
    >
      <div class="rd-waive">
        <p>{{ t("s04.tasks.readiness.waiveDesc", { code: waiveTarget }) }}</p>
        <Input
          v-model="waiveReason"
          :placeholder="t('s04.tasks.readiness.waivePlaceholder')"
          data-testid="rd-waive-reason"
        />
        <p v-if="waiveError" class="rd-waive__error">{{ waiveError }}</p>
      </div>
      <template #footer>
        <Button variant="ghost" size="sm" @click="waiveTarget = ''">
          {{ t("s04.tasks.common.cancel") }}
        </Button>
        <Button
          variant="primary"
          size="sm"
          data-testid="rd-waive-confirm"
          @click="confirmWaive"
        >
          {{ t("s04.tasks.readiness.waiveConfirm") }}
        </Button>
      </template>
    </Dialog>

    <ConfirmationDialog
      :open="handoffConfirmOpen"
      :title="t('s04.tasks.readiness.handoffConfirmTitle')"
      :description="handoffDescription"
      :confirm-label="t('s04.tasks.readiness.handoffConfirmBtn')"
      :cancel-label="t('s04.tasks.common.cancel')"
      @confirm="confirmHandoff"
      @cancel="handoffConfirmOpen = false"
    />
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

.rd-cat ul {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rd-cat li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
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

.rd-item-text {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
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

.rd-row-state--attention {
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

.rd-issue.is-closed {
  opacity: 0.65;
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

.rd-issue__severity--low {
  border-color: var(--ws-border);
  color: var(--ws-text-secondary);
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

.rd-issue__state {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.rd-issue__resolution {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.rd-issue__actions {
  display: flex;
  gap: var(--space-2);
}

.rd-recs {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rd-recs li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border: var(--border-1) dashed var(--ws-color-info);
  border-radius: var(--radius-sm);
}

.rd-recs p {
  flex: 1;
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
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

.rd-waive {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rd-waive p {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.rd-waive__error {
  margin: 0;
  color: var(--ws-color-danger);
  font-size: var(--font-size-caption);
}

@media (max-width: 1100px) {
  .rd-summary,
  .rd-checklist {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
