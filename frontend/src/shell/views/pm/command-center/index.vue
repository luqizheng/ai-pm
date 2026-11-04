<script setup lang="ts">
/**
 * 项目总控台（S01 · I5-D）。
 *
 * 首屏优先级（I5 V1.1 §3.1）：待人工处理 → 当前执行 → 风险与阻塞
 *   → 项目目标与进展 → 初始化进度 → 最近活动。
 *
 * 规则：
 * - 块 `status="unavailable"` → 显示「数据暂不可用」，**不得**显示 0 或空列表；
 * - 空集合 → 显示该块的空状态文案（与不可用严格区分）；
 * - 初始化进度**不展示百分比**；完成度 `not_determinable` 时不宣称已完成；
 * - 归档项目 → 只读态提示，不提供新活动入口。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { PageHeader } from "@/shell/components";
import { formatDateTime } from "@/api/format";
import {
  getCommandCenter,
  type ActivityItem,
  type ActionItem,
  type CommandCenter,
  type CommandCenterBlock,
  type InitItem,
  type RiskItem
} from "@/api/command-center";

const { t } = useI18n();
const route = useRoute();

const projectId = computed(() => String(route.params.projectId ?? ""));
const center = ref<CommandCenter | null>(null);
const loading = ref(false);
const loadFailed = ref(false);
const activityOffset = ref(0);
const activityLimit = 20;

async function load() {
  loading.value = true;
  loadFailed.value = false;
  try {
    center.value = await getCommandCenter(projectId.value, {
      activity_limit: activityLimit,
      activity_offset: activityOffset.value
    });
  } catch {
    loadFailed.value = true;
  } finally {
    loading.value = false;
  }
}

function block<T>(name: keyof CommandCenter["blocks"]) {
  return center.value?.blocks?.[name] as CommandCenterBlock<T> | undefined;
}

/** 块的可用数据；不可用时返回 undefined（页面据此显示「暂不可用」而不是 0）。 */
function data<T>(name: keyof CommandCenter["blocks"]): T | undefined {
  const b = block<T>(name);
  return b?.status === "ok" ? b.data : undefined;
}

const summary = computed(() => data<CommandCenter["blocks"]["project_summary"]["data"]>("project_summary"));
const init = computed(() => data<CommandCenter["blocks"]["initialization_progress"]["data"]>("initialization_progress"));
const work = computed(() => data<CommandCenter["blocks"]["work_summary"]["data"]>("work_summary"));
const actions = computed(() => data<CommandCenter["blocks"]["human_action_summary"]["data"]>("human_action_summary"));
const risks = computed(() => data<CommandCenter["blocks"]["risk_summary"]["data"]>("risk_summary"));
const activity = computed(() => data<CommandCenter["blocks"]["recent_activity"]["data"]>("recent_activity"));

function initStatusVariant(status: string) {
  switch (status) {
    case "completed":
      return "success" as const;
    case "blocked":
      return "danger" as const;
    case "in_progress":
      return "warning" as const;
    default:
      return "neutral" as const;
  }
}

function severityVariant(severity: string) {
  switch (severity) {
    case "high":
      return "danger" as const;
    case "medium":
      return "warning" as const;
    default:
      return "neutral" as const;
  }
}

function actionKindLabel(kind: string) {
  const map: Record<string, string> = {
    finding_pending: "pm.commandCenter.action.findingPending",
    finding_revision_requested: "pm.commandCenter.action.findingRevision",
    proposal_pending: "pm.commandCenter.action.proposalPending",
    delivery_pending: "pm.commandCenter.action.deliveryPending"
  };
  return t(map[kind] ?? "pm.commandCenter.action.other");
}

function initStatusLabel(status: string) {
  const map: Record<string, string> = {
    not_started: "pm.commandCenter.init.notStarted",
    in_progress: "pm.commandCenter.init.inProgress",
    completed: "pm.commandCenter.init.completed",
    blocked: "pm.commandCenter.init.blocked",
    not_applicable: "pm.commandCenter.init.notApplicable",
    undetermined: "pm.commandCenter.init.undetermined"
  };
  return t(map[status] ?? "pm.commandCenter.init.unknown");
}

function activityLabel(type: string) {
  const map: Record<string, string> = {
    project_archived: "pm.commandCenter.activity.projectArchived",
    goal_committed: "pm.commandCenter.activity.goalCommitted",
    finding_decided: "pm.commandCenter.activity.findingDecided",
    finding_revision_requested: "pm.commandCenter.activity.findingRevision",
    repository_created: "pm.commandCenter.activity.repositoryCreated",
    repository_verified: "pm.commandCenter.activity.repositoryVerified",
    analysis_succeeded: "pm.commandCenter.activity.analysisSucceeded",
    analysis_failed: "pm.commandCenter.activity.analysisFailed",
    run_started: "pm.commandCenter.activity.runStarted",
    run_finished: "pm.commandCenter.activity.runFinished",
    run_failed: "pm.commandCenter.activity.runFailed",
    run_terminated: "pm.commandCenter.activity.runTerminated",
    delivery_submitted: "pm.commandCenter.activity.deliverySubmitted",
    delivery_rejected: "pm.commandCenter.activity.deliveryRejected",
    review_completed: "pm.commandCenter.activity.reviewCompleted",
    review_changes_requested: "pm.commandCenter.activity.reviewChangesRequested",
    member_changed: "pm.commandCenter.activity.memberChanged",
    device_bound: "pm.commandCenter.activity.deviceBound"
  };
  return t(map[type] ?? "pm.commandCenter.activity.other");
}

function agentLabel(agent?: { display_name: string; kind: string }) {
  if (!agent) return t("pm.commandCenter.noAgent");
  const kindKey =
    agent.kind === "human"
      ? "pm.commandCenter.actorHuman"
      : agent.kind === "unknown"
        ? "pm.commandCenter.actorUnknown"
        : "pm.commandCenter.actorAgent";
  return `${agent.display_name} · ${t(kindKey)}`;
}

function detailOf(blockName: keyof CommandCenter["blocks"]) {
  return block(blockName)?.error?.message ?? t("pm.commandCenter.unavailable");
}

function activityItems(): ActivityItem[] {
  return activity.value?.items ?? [];
}

function actionItems(): ActionItem[] {
  return actions.value?.items ?? [];
}

function riskItems(): RiskItem[] {
  return risks.value?.items ?? [];
}

function initItems(): InitItem[] {
  return init.value?.items ?? [];
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('pm.commandCenter.pageTitle')"
      :subtitle="summary?.name ?? t('pm.commandCenter.subtitle')"
    />

    <Alert
      v-if="loadFailed"
      variant="danger"
      :title="t('pm.commandCenter.loadFailed')"
    />

    <Alert
      v-if="summary?.archived"
      variant="info"
      :title="t('pm.commandCenter.archivedTitle')"
      :description="t('pm.commandCenter.archivedHint')"
    />

    <!-- ① 项目头部 -->
    <Card>
      <template #header>{{ t("pm.commandCenter.section.summary") }}</template>
      <div v-if="!summary" class="block-unavailable">
        {{ detailOf("project_summary") }}
      </div>
      <dl v-else class="summary-grid">
        <div>
          <dt>{{ t("pm.commandCenter.field.code") }}</dt>
          <dd>{{ summary.code }}</dd>
        </div>
        <div>
          <dt>{{ t("pm.commandCenter.field.lifecycle") }}</dt>
          <dd>{{ summary.lifecycle }}</dd>
        </div>
        <div>
          <dt>{{ t("pm.commandCenter.field.repositories") }}</dt>
          <dd>
            {{ summary.repository_count }}
            <span class="cell-hint">
              （{{ summary.verified_repository_count }}
              {{ t("pm.commandCenter.field.verified") }}）
            </span>
          </dd>
        </div>
        <div>
          <dt>{{ t("pm.commandCenter.field.members") }}</dt>
          <dd>{{ summary.member_count }}</dd>
        </div>
        <div>
          <dt>{{ t("pm.commandCenter.field.goal") }}</dt>
          <dd>
            <span v-if="!summary.goal.present">{{
              t("pm.commandCenter.goal.missing")
            }}</span>
            <span v-else>
              {{ t("pm.commandCenter.goal.revision", { no: summary.goal.revision_no }) }}
              <Badge v-if="summary.goal.pending_proposal" variant="warning">
                {{ t("pm.commandCenter.goal.pending") }}
              </Badge>
            </span>
          </dd>
        </div>
        <div>
          <dt>{{ t("pm.commandCenter.field.lastActivity") }}</dt>
          <dd>
            {{
              summary.last_activity_at
                ? formatDateTime(summary.last_activity_at)
                : t("pm.commandCenter.none")
            }}
          </dd>
        </div>
      </dl>
    </Card>

    <!-- ② 待人工处理（首屏第一优先级） -->
    <Card>
      <template #header>{{ t("pm.commandCenter.section.actions") }}</template>
      <div v-if="!actions" class="block-unavailable">
        {{ detailOf("human_action_summary") }}
      </div>
      <p v-else-if="actionItems().length === 0" class="empty-hint">
        {{ t("pm.commandCenter.empty.actions") }}
      </p>
      <ul v-else class="item-list">
        <li v-for="item in actionItems()" :key="`${item.domain}-${item.kind}-${item.id}`">
          <div class="item-head">
            <Badge variant="warning">{{ actionKindLabel(item.kind) }}</Badge>
            <span class="item-title">{{ item.title }}</span>
          </div>
          <p v-if="item.detail" class="cell-hint">{{ item.detail }}</p>
          <p class="cell-hint">
            {{ t("pm.commandCenter.field.sourceState") }}: {{ item.source_state }}
            · {{ item.action_hint }}
          </p>
        </li>
      </ul>
    </Card>

    <!-- ③ 当前执行 -->
    <Card>
      <template #header>{{ t("pm.commandCenter.section.work") }}</template>
      <div v-if="!work" class="block-unavailable">
        {{ detailOf("work_summary") }}
      </div>
      <template v-else>
        <p class="cell-hint">
          {{ t("pm.commandCenter.work.tasks") }}: {{ work.task_total }}
          · {{ t("pm.commandCenter.work.activeRuns") }}: {{ work.active_run_count }}
        </p>
        <p v-if="work.active_runs.length === 0" class="empty-hint">
          {{ t("pm.commandCenter.empty.runs") }}
        </p>
        <ul v-else class="item-list">
          <li v-for="run in work.active_runs" :key="run.run_id">
            <div class="item-head">
              <Badge variant="info">{{ run.status }}</Badge>
              <span class="item-title">{{ run.task_title }}</span>
            </div>
            <p class="cell-hint">
              {{ agentLabel(run.agent) }}
              · {{ t("pm.commandCenter.field.attempt") }} {{ run.attempt_no }}
              <template v-if="run.repositories.length">
                · {{ run.repositories.length }}
                {{ t("pm.commandCenter.field.repositoriesShort") }}
              </template>
            </p>
          </li>
        </ul>
      </template>
    </Card>

    <!-- ④ 风险与阻塞 -->
    <Card>
      <template #header>{{ t("pm.commandCenter.section.risks") }}</template>
      <div v-if="!risks" class="block-unavailable">
        {{ detailOf("risk_summary") }}
      </div>
      <p v-else-if="riskItems().length === 0" class="empty-hint">
        {{ t("pm.commandCenter.empty.risks") }}
      </p>
      <ul v-else class="item-list">
        <li v-for="item in riskItems()" :key="`${item.kind}-${item.source_id}`">
          <div class="item-head">
            <Badge :variant="severityVariant(item.severity)">{{ item.severity }}</Badge>
            <span class="item-title">{{ item.title }}</span>
          </div>
          <p class="cell-hint">
            {{ t("pm.commandCenter.field.sourceState") }}: {{ item.source_state }}
            · {{ item.suggestion }}
          </p>
        </li>
      </ul>
    </Card>

    <!-- ⑤ 初始化进度（无百分比） -->
    <Card>
      <template #header>{{ t("pm.commandCenter.section.init") }}</template>
      <div v-if="!init" class="block-unavailable">
        {{ detailOf("initialization_progress") }}
      </div>
      <template v-else>
        <p v-if="init.completion === 'not_determinable'" class="cell-hint">
          {{ t("pm.commandCenter.init.notDeterminable") }}
        </p>
        <ul class="item-list">
          <li v-for="item in initItems()" :key="item.key">
            <div class="item-head">
              <Badge :variant="initStatusVariant(item.status)">
                {{ initStatusLabel(item.status) }}
              </Badge>
              <span class="item-title">{{ item.title }}</span>
              <span v-if="item.required" class="cell-hint">
                {{ t("pm.commandCenter.init.required") }}
              </span>
            </div>
            <p v-if="item.detail" class="cell-hint">{{ item.detail }}</p>
          </li>
        </ul>
      </template>
    </Card>

    <!-- ⑥ 最近活动 -->
    <Card>
      <template #header>{{ t("pm.commandCenter.section.activity") }}</template>
      <div v-if="!activity" class="block-unavailable">
        {{ detailOf("recent_activity") }}
      </div>
      <p v-else-if="activityItems().length === 0" class="empty-hint">
        {{ t("pm.commandCenter.empty.activity") }}
      </p>
      <ul v-else class="item-list">
        <li v-for="item in activityItems()" :key="item.id">
          <div class="item-head">
            <Badge variant="neutral">{{ activityLabel(item.activity_type) }}</Badge>
            <span class="item-title">{{ item.summary }}</span>
          </div>
          <p class="cell-hint">
            {{ item.actor.name ?? t("pm.commandCenter.actorUnknown") }}
            · {{ formatDateTime(item.occurred_at) }}
          </p>
        </li>
      </ul>
      <template v-if="activity">
        <div class="pager">
          <Button
            size="sm"
            variant="secondary"
            :disabled="activityOffset === 0"
            @click="
              activityOffset = Math.max(0, activityOffset - activityLimit);
              load();
            "
          >
            {{ t("pm.commandCenter.prevPage") }}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            :disabled="activityOffset + activityLimit >= activity.total"
            @click="
              activityOffset = activityOffset + activityLimit;
              load();
            "
          >
            {{ t("pm.commandCenter.nextPage") }}
          </Button>
        </div>
      </template>
    </Card>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: var(--space-4);
  margin: 0;
}

.summary-grid dt {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.summary-grid dd {
  margin: var(--space-1) 0 0;
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.item-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.item-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.item-title {
  font-weight: var(--weight-medium);
}

.cell-hint {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
  margin: var(--space-1) 0 0;
}

.empty-hint {
  color: var(--text-secondary);
  margin: 0;
}

.block-unavailable {
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
}

.pager {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-4);
}
</style>
