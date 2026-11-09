<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check, Plus, Sparkles, X } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import {
  breakdownBaseline,
  breakdownFeatures,
  breakdownProposedTasks,
  type BreakdownTaskFixture
} from "./fixtures";

/**
 * 任务拆解页根（S04-UI-02，mock 预览）。
 *
 * AI 拆解接口未接线（pm-ai 的 decompose 仅 worker 侧触发），本页演示
 * S04-UI-02 的核心交互模型：选基线 → 生成提案（分批流式）→ 逐条/批量
 * 评审（Accept / Reject / 追加子任务）→ 审批汇总。提案只存在于本页会话；
 * 路由仅在 VITE_USE_MOCK=1 注册真实页，发布构建走占位页（release-guard）。
 *
 * 交互模型与未来接真契约对齐：AI 生成的任务一律是「提案」（pending），
 * 人类裁决后才成为正式任务；AI 不得直接落 ready（E2 闸门在服务端）。
 */
const { t } = useI18n();

type ProposalState = "pending" | "accepted" | "rejected";

const baseline = breakdownBaseline;
const features = breakdownFeatures;

const proposals = ref<BreakdownTaskFixture[]>([]);
const states = ref<Record<string, ProposalState>>({});
const selectedCodes = ref<Set<string>>(new Set());
const selectedCode = ref("");
const generating = ref(false);
const addedSubtasks = ref(0);
let generateTimer: ReturnType<typeof setTimeout> | undefined;

onBeforeUnmount(() => {
  if (generateTimer) clearTimeout(generateTimer);
});

async function generate() {
  if (generating.value) return;
  generating.value = true;
  proposals.value = [];
  states.value = {};
  selectedCodes.value = new Set();
  selectedCode.value = "";
  addedSubtasks.value = 0;
  // 模拟流式生成：分批落行；接口接线后由 SSE/轮询替代
  const tasks = breakdownProposedTasks();
  const chunkSize = 3;
  for (let index = 0; index < tasks.length; index += chunkSize) {
    await new Promise(resolve => {
      generateTimer = setTimeout(resolve, 120);
    });
    proposals.value.push(...tasks.slice(index, index + chunkSize));
  }
  generating.value = false;
}

function stateOf(code: string): ProposalState {
  return states.value[code] ?? "pending";
}

function stateLabel(code: string): string {
  const state = stateOf(code);
  if (state === "accepted") return t("s04.tasks.breakdown.stateAccepted");
  if (state === "rejected") return t("s04.tasks.breakdown.stateRejected");
  return t("s04.tasks.breakdown.proposalBadge");
}

function rowClass(code: string) {
  return {
    "is-proposal": stateOf(code) === "pending",
    "is-accepted": stateOf(code) === "accepted",
    "is-rejected": stateOf(code) === "rejected"
  };
}

function isProposal(code: string): boolean {
  return !states.value[code] || states.value[code] === "pending";
}

function setStates(codes: string[], state: ProposalState) {
  const next = { ...states.value };
  for (const code of codes) next[code] = state;
  states.value = next;
}

function accept(codes: string[]) {
  setStates(codes, "accepted");
  selectedCodes.value = new Set();
}

function reject(codes: string[]) {
  setStates(codes, "rejected");
  selectedCodes.value = new Set();
}

const selectedPending = computed(() =>
  [...selectedCodes.value].filter(code => isProposal(code))
);

function toggleSelected(code: string, checked: boolean) {
  const next = new Set(selectedCodes.value);
  if (checked) next.add(code);
  else next.delete(code);
  selectedCodes.value = next;
}

function onRowCheck(code: string, event: Event) {
  toggleSelected(code, (event.target as HTMLInputElement).checked);
}

const summary = computed(() => {
  const counts = {
    proposals: proposals.value.length,
    pending: 0,
    accepted: 0,
    rejected: 0
  };
  for (const task of proposals.value) {
    counts[stateOf(task.code)] += 1;
  }
  return counts;
});

const allReviewed = computed(
  () => summary.value.proposals > 0 && summary.value.pending === 0
);

function addSubtask(parent: BreakdownTaskFixture) {
  addedSubtasks.value += 1;
  proposals.value.push({
    ...parent,
    code: `${parent.code}.${parent.subtaskSuggestions.length + addedSubtasks.value}`,
    parentId: parent.code,
    title: `${t("s04.tasks.breakdown.addSubtask")} ${addedSubtasks.value}`,
    deps: 0,
    subtaskSuggestions: [],
    criteria: []
  });
}

function selectForPanel(task: BreakdownTaskFixture) {
  selectedCode.value = task.code;
}

const panelTask = computed(
  () => proposals.value.find(task => task.code === selectedCode.value) ?? null
);

function childrenOf(code: string | null) {
  return proposals.value.filter(task => task.parentId === code);
}

function featureTasks(featureId: string) {
  return proposals.value.filter(
    task => task.featureId === featureId && task.parentId === null
  );
}

function rowsFor(task: BreakdownTaskFixture) {
  return [task, ...childrenOf(task.code)];
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
        variant="primary"
        size="sm"
        :disabled="generating"
        data-testid="bd-generate"
        @click="generate"
      >
        <Sparkles :size="14" />
        {{
          generating
            ? t("s04.tasks.breakdown.generating")
            : proposals.length > 0
              ? t("s04.tasks.breakdown.regenerate")
              : t("s04.tasks.breakdown.generate")
        }}
      </Button>
    </template>

    <TasksFrame />

    <Alert variant="info" :title="t('s04.tasks.breakdown.mockNote')" />

    <section class="bd-baseline">
      <header class="bd-baseline__head">
        <h2 class="bd-baseline__title">
          {{ t("s04.tasks.breakdown.baselineTitle") }}
        </h2>
        <span class="bd-baseline__status" data-testid="bd-baseline-status">
          {{ t("s04.tasks.breakdown.baselineStatus") }}
        </span>
        <Button variant="ghost" size="sm">{{
          t("s04.tasks.breakdown.viewSource")
        }}</Button>
      </header>
      <p class="bd-baseline__name">{{ baseline.name }}</p>
      <p class="bd-baseline__scope">{{ baseline.scope }}</p>
      <div class="bd-baseline__chips">
        <span v-for="req in baseline.requirements" :key="req" class="bd-chip">{{
          req
        }}</span>
        <span
          v-for="design in baseline.designs"
          :key="design"
          class="bd-chip"
          >{{ design }}</span
        >
        <span v-for="adr in baseline.adrs" :key="adr" class="bd-chip">{{
          adr
        }}</span>
      </div>
    </section>

    <div class="bd-workspace">
      <section class="bd-main">
        <header class="bd-main__head">
          <h2 class="bd-main__title">
            {{ t("s04.tasks.breakdown.treeTitle") }}
          </h2>
          <span
            v-if="proposals.length > 0"
            class="bd-summary"
            data-testid="bd-summary"
          >
            {{
              t("s04.tasks.breakdown.summaryBar", {
                proposals: summary.proposals,
                pending: summary.pending,
                accepted: summary.accepted,
                rejected: summary.rejected
              })
            }}
          </span>
        </header>

        <div
          v-if="selectedPending.length > 0"
          class="bd-bulk"
          data-testid="bd-bulk"
        >
          <span>{{
            t("s04.tasks.common.selectedCount", {
              count: selectedPending.length
            })
          }}</span>
          <Button
            variant="primary"
            size="sm"
            data-testid="bd-accept-selected"
            @click="accept(selectedPending)"
          >
            <Check :size="13" />
            {{ t("s04.tasks.breakdown.acceptSelected") }}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            data-testid="bd-reject-selected"
            @click="reject(selectedPending)"
          >
            <X :size="13" />
            {{ t("s04.tasks.breakdown.rejectSelected") }}
          </Button>
          <Button variant="ghost" size="sm" @click="selectedCodes = new Set()">
            {{ t("s04.tasks.breakdown.clearSelection") }}
          </Button>
        </div>

        <EmptyState
          v-if="proposals.length === 0 && !generating"
          :title="t('s04.tasks.common.empty')"
          :description="t('s04.tasks.breakdown.requiresBaseline')"
          size="sm"
        />
        <p v-if="generating" class="bd-generating" data-testid="bd-generating">
          {{ t("s04.tasks.breakdown.generating") }}
        </p>

        <div v-for="feature in features" :key="feature.id" class="bd-feature">
          <p class="bd-feature__title">
            {{ t("s04.tasks.breakdown.featureLabel") }} · {{ feature.title }}
          </p>
          <div
            v-for="task in featureTasks(feature.id)"
            :key="task.code"
            class="bd-node"
          >
            <div
              class="bd-row bd-row--feature"
              :class="[
                rowClass(task.code),
                { 'is-selected': selectedCode === task.code }
              ]"
              :data-testid="`bd-row-${task.code}`"
              :data-state="stateOf(task.code)"
              role="button"
              tabindex="0"
              @click="selectForPanel(task)"
              @keydown.enter="selectForPanel(task)"
            >
              <input
                type="checkbox"
                :checked="selectedCodes.has(task.code)"
                :aria-label="task.title"
                @change="onRowCheck(task.code, $event)"
              />
              <span class="bd-row__code">{{ task.code }}</span>
              <span class="bd-row__title">{{ task.title }}</span>
              <TaskPriorityChip :priority="task.priority" />
              <span class="bd-row__meta"
                >{{ task.effort }} · {{ task.repo }}</span
              >
              <span class="bd-row__state">
                {{ stateLabel(task.code) }}
              </span>
              <span class="bd-row__ops">
                <Button
                  v-if="isProposal(task.code)"
                  variant="ghost"
                  size="sm"
                  :data-testid="`bd-accept-${task.code}`"
                  @click.stop="accept([task.code])"
                >
                  {{ t("s04.tasks.breakdown.accept") }}
                </Button>
                <Button
                  v-if="isProposal(task.code)"
                  variant="ghost"
                  size="sm"
                  @click.stop="reject([task.code])"
                >
                  {{ t("s04.tasks.breakdown.reject") }}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  @click.stop="addSubtask(task)"
                >
                  <Plus :size="13" />
                  {{ t("s04.tasks.breakdown.addSubtask") }}
                </Button>
              </span>
            </div>
            <div
              v-for="child in rowsFor(task).slice(1)"
              :key="child.code"
              class="bd-row bd-row--child"
              :class="[
                rowClass(child.code),
                { 'is-selected': selectedCode === child.code }
              ]"
              :data-testid="`bd-row-${child.code}`"
              :data-state="stateOf(child.code)"
              role="button"
              tabindex="0"
              @click="selectForPanel(child)"
              @keydown.enter="selectForPanel(child)"
            >
              <input
                type="checkbox"
                :checked="selectedCodes.has(child.code)"
                :aria-label="child.title"
                @change="onRowCheck(child.code, $event)"
              />
              <span class="bd-row__code">{{ child.code }}</span>
              <span class="bd-row__title">{{ child.title }}</span>
              <TaskPriorityChip :priority="child.priority" />
              <span class="bd-row__meta"
                >{{ child.effort }} · {{ child.repo }}</span
              >
              <span class="bd-row__state">
                {{ stateLabel(child.code) }}
              </span>
              <span class="bd-row__ops">
                <Button
                  v-if="isProposal(child.code)"
                  variant="ghost"
                  size="sm"
                  @click.stop="accept([child.code])"
                >
                  {{ t("s04.tasks.breakdown.accept") }}
                </Button>
                <Button
                  v-if="isProposal(child.code)"
                  variant="ghost"
                  size="sm"
                  @click.stop="reject([child.code])"
                >
                  {{ t("s04.tasks.breakdown.reject") }}
                </Button>
              </span>
            </div>
          </div>
        </div>

        <Alert
          v-if="allReviewed"
          variant="success"
          :title="t('s04.tasks.breakdown.allReviewedTitle')"
          data-testid="bd-all-reviewed"
        >
          {{ t("s04.tasks.breakdown.allReviewedDesc") }}
        </Alert>
      </section>

      <aside class="bd-panel">
        <h2 class="bd-panel__title">
          <Sparkles :size="14" />
          {{ t("s04.tasks.breakdown.aiPanelTitle") }}
        </h2>
        <template v-if="panelTask">
          <section class="bd-panel__section">
            <h3>{{ t("s04.tasks.breakdown.aiWhy") }}</h3>
            <p>{{ panelTask.why }}</p>
          </section>
          <section class="bd-panel__section">
            <h3>{{ t("s04.tasks.breakdown.aiTrace") }}</h3>
            <ul class="bd-panel__list">
              <li>
                <span>{{ t("s04.tasks.detail.traceRequirement") }}</span
                >{{ panelTask.trace.requirement }}
              </li>
              <li>
                <span>{{ t("s04.tasks.detail.traceDesign") }}</span
                >{{ panelTask.trace.design }}
              </li>
              <li>
                <span>{{ t("s04.tasks.detail.traceAdr") }}</span
                >{{ panelTask.trace.adr }}
              </li>
              <li>
                <span>{{ t("s04.tasks.detail.traceBaseline") }}</span
                >{{ panelTask.trace.baseline }}
              </li>
            </ul>
          </section>
          <section class="bd-panel__section">
            <h3>{{ t("s04.tasks.breakdown.aiMissing") }}</h3>
            <p>{{ panelTask.missing }}</p>
          </section>
          <section class="bd-panel__section">
            <h3>{{ t("s04.tasks.breakdown.aiOverlap") }}</h3>
            <p>{{ panelTask.overlap }}</p>
          </section>
          <section
            v-if="panelTask.subtaskSuggestions.length > 0"
            class="bd-panel__section"
          >
            <h3>{{ t("s04.tasks.breakdown.aiSubtasks") }}</h3>
            <ul class="bd-panel__list">
              <li
                v-for="subtask in panelTask.subtaskSuggestions"
                :key="subtask"
              >
                {{ subtask }}
              </li>
            </ul>
          </section>
          <section v-if="panelTask.risks.length > 0" class="bd-panel__section">
            <h3>{{ t("s04.tasks.breakdown.aiRisks") }}</h3>
            <ul class="bd-panel__list">
              <li v-for="risk in panelTask.risks" :key="risk">{{ risk }}</li>
            </ul>
          </section>
          <section
            v-if="panelTask.criteria.length > 0"
            class="bd-panel__section"
          >
            <h3>{{ t("s04.tasks.breakdown.aiCriteria") }}</h3>
            <ul class="bd-panel__list">
              <li v-for="criterion in panelTask.criteria" :key="criterion">
                {{ criterion }}
              </li>
            </ul>
          </section>
        </template>
        <EmptyState
          v-else
          :title="t('s04.tasks.breakdown.aiPanelTitle')"
          :description="t('s04.tasks.breakdown.requiresBaseline')"
          size="sm"
        />
      </aside>
    </div>
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
  gap: var(--space-3);
}

.bd-baseline__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.bd-baseline__status {
  padding: 0 var(--space-2);
  height: var(--size-control-sm);
  display: inline-flex;
  align-items: center;
  border-radius: var(--radius-full);
  border: var(--border-1) solid var(--ws-color-success);
  color: var(--ws-color-success);
  font-size: var(--font-size-caption);
}

.bd-baseline__name {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.bd-baseline__scope {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.bd-baseline__chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.bd-chip {
  padding: 0 var(--space-2);
  height: var(--size-control-sm);
  display: inline-flex;
  align-items: center;
  border-radius: var(--radius-full);
  background: var(--ws-elevated);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.bd-workspace {
  display: grid;
  grid-template-columns: minmax(0, 62fr) minmax(0, 38fr);
  gap: var(--space-5);
  align-items: start;
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

.bd-bulk {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  margin-bottom: var(--space-3);
  border: var(--border-1) solid var(--ws-accent);
  border-radius: var(--radius-sm);
  background: var(--ws-color-accent-surface);
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.bd-generating {
  margin: 0;
  color: var(--ws-color-info);
  font-size: var(--font-size-caption);
}

.bd-feature {
  margin-bottom: var(--space-4);
}

.bd-feature__title {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
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
}

.bd-row--child {
  margin-left: var(--space-8);
}

/* AI 提案行：Info 蓝虚线左缘 + 低饱和底（禁紫色，V2 裁决） */
.bd-row.is-proposal {
  border-left: calc(var(--space-1) / 2) dashed var(--ws-color-info);
  background: var(--ws-color-accent-surface);
}

.bd-row.is-accepted {
  border-left: calc(var(--space-1) / 2) solid var(--ws-color-success);
}

.bd-row.is-rejected {
  border-left: calc(var(--space-1) / 2) solid var(--ws-border);
}

.bd-row.is-rejected .bd-row__title {
  color: var(--ws-text-disabled);
  text-decoration: line-through;
}

.bd-row.is-selected {
  outline: var(--size-focus-ring) solid var(--ws-accent);
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

.bd-row__state--accepted {
  color: var(--ws-color-success);
}

.bd-row__state--rejected {
  color: var(--ws-text-disabled);
  text-decoration: line-through;
}

.bd-row__ops {
  display: flex;
  gap: var(--space-1);
  white-space: nowrap;
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

.bd-panel__list {
  margin: 0;
  padding-left: var(--space-4);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.bd-panel__list li > span {
  display: inline-block;
  min-width: var(--space-12);
  margin-right: var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

@media (max-width: 1100px) {
  .bd-workspace {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
