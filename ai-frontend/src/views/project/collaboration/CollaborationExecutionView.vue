<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { translateError } from "@/api/base";
import { listTasks } from "@/api/pm";
import { getTaskDetail, type TaskDetailView } from "@/api/task-plans";
import { listAgents } from "@/api/execution-plans";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import ExecutionStateBadge from "./components/ExecutionStateBadge.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 B — AI execution detail（Figma 259:165，Execution 页签）。
 *
 * 任务级执行详情：眉行（code/标题）+ 执行计划卡（PA-06 执行状态 7 态链，
 * 当前态高亮——画布的「6 of 8 steps」链是编造性步骤，V1 用真实状态机链）
 * + 计划与依赖（上游/下游分解 + 产物与验证）+ ACTIVITY·PROJECT RECORD
 * （任务历史）。右栏 = Why AI is doing this（目标与依据 / 变更史 /
 * 边界内 / Human control）。
 *
 * 诚实缺位：Run 级步骤步数后端无读面（画布「6 OF 8 STEPS」不实现）；
 * Pause task 按钮后端无通用暂停端点——不渲染按钮，能力缺口如实标注。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);
const openTasks = ref<Array<{ id: string; code: string; title: string }>>([]);
const selectedTaskId = ref<string>("");
const detail = ref<TaskDetailView | null>(null);
const agentName = ref<string | null>(null);

const OPEN_STATES = new Set([
  "ready",
  "running",
  "reviewing",
  "verifying",
  "blocked",
  "waiting"
]);

const STATE_CHAIN = ["planned", "executing", "verifying", "completed"] as const;

const chainPosition = computed(() => {
  const status = detail.value?.task.status ?? "";
  if (status === "draft" || status === "planned") return 0;
  if (status === "ready" || status === "running") return 1;
  if (status === "reviewing" || status === "verifying") return 2;
  if (status === "completed") return 3;
  return -1; // waiting/blocked/failed 不在线性链上，如实现状徽标
});

const taskOptions = computed(() =>
  openTasks.value.map(task => ({
    value: task.id,
    label: `${task.code} · ${task.title}`
  }))
);

const goalLine = computed(() => {
  const task = detail.value?.task;
  if (!task) return "";
  return task.goal?.trim() || task.description?.trim() || task.title;
});

const tracesText = computed(() => {
  const traces = detail.value?.traces ?? [];
  if (traces.length === 0) return null;
  return traces
    .map(
      trace =>
        `${trace.code ?? trace.kind} · ${trace.title ?? ""}${
          trace.version ? ` · ${trace.version}` : ""
        }`
    )
    .join("\n");
});

const dependencies = computed(() => detail.value?.dependencies ?? []);

const criteriaStats = computed(() => {
  const criteria = detail.value?.criteria ?? [];
  return t("pa06.execution.criteriaCount", { count: criteria.length });
});

const deliverableLines = computed(() => {
  const task = detail.value?.task;
  const deliverables = (task?.deliverables ?? []) as Array<{
    path?: string;
  }>;
  const repos = detail.value?.repositories ?? [];
  const lines: string[] = [];
  if (task?.git_branch) {
    lines.push(t("pa06.execution.branch", { branch: task.git_branch }));
  }
  if (repos.length > 0) {
    lines.push(t("pa06.execution.repos", { count: repos.length }));
  }
  if (deliverables.length > 0) {
    lines.push(
      t("pa06.execution.deliverables", { count: deliverables.length })
    );
  }
  lines.push(criteriaStats.value);
  return lines;
});

const historyLines = computed(() => (detail.value?.history ?? []).slice(0, 4));

const latestHistory = computed(() => detail.value?.history?.[0] ?? null);

function fmtTime(iso: string | null): string {
  return iso ? new Date(iso).toLocaleTimeString() : "—";
}

async function load(): Promise<void> {
  if (!hasProject.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const id = projectId.value;
    const fromQuery = route.query.task_id;
    const rows = await listTasks({ project_id: id, page: 1, size: 200 });
    const all = rows.items ?? [];
    openTasks.value = all
      .filter(task => OPEN_STATES.has(task.status))
      .map(task => ({ id: task.id, code: task.code, title: task.title }));
    const wanted =
      typeof fromQuery === "string" && fromQuery
        ? fromQuery
        : (openTasks.value[0]?.id ?? "");
    selectedTaskId.value = wanted;
    if (wanted) {
      const [taskDetail, roster] = await Promise.all([
        getTaskDetail(wanted),
        listAgents(id).catch(() => [])
      ]);
      detail.value = taskDetail;
      const assignee = taskDetail.task.assignee_id;
      agentName.value = assignee
        ? (roster.find(row => row.id === assignee)?.name ?? null)
        : null;
    } else {
      detail.value = null;
      agentName.value = null;
    }
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

function pickTask(value: string): void {
  void router.replace({
    path: "/project/collaboration/execution",
    query: { project_id: projectId.value, task_id: value }
  });
}

function openVerification(): void {
  void router.push("/project/collaboration/delivery");
}

function inspectRecovery(): void {
  void router.push("/project/collaboration/recovery");
}

onMounted(load);
watch([projectId, () => route.query.task_id], load);
</script>

<template>
  <CollaborationFrame>
    <ScreenFeedback
      v-if="loading"
      state="loading"
      :title="$t('pa06.execution.loadingEyebrow')"
    />
    <ScreenFeedback
      v-else-if="error"
      state="error"
      :description="error"
      @retry="load"
    />
    <template v-else>
      <template v-if="!hasProject">
        <ScreenFeedback state="empty" :title="$t('pa06.frame.noProject')" />
      </template>
      <!-- 无进行中执行：诚实空态（画布 Empty · clear next step） -->
      <ScreenFeedback
        v-else-if="openTasks.length === 0"
        state="empty"
        :title="$t('pa06.execution.emptyTitle')"
        :description="$t('pa06.execution.emptyHint')"
      />
      <CollabLayout v-else>
        <div class="ex-head">
          <div class="ex-head__eyebrow-row">
            <span class="ex-eyebrow">
              {{ detail?.task.code }} · {{ detail?.task.title }}
            </span>
            <ExecutionStateBadge v-if="detail" :status="detail.task.status" />
          </div>
          <h2 class="ex-head__title">{{ goalLine }}</h2>
          <p class="ex-head__meta">
            {{ agentName ?? $t("pa06.overview.agentFallback") }} ·
            {{ $t("pa06.overview.coordinatedBy") }} ·
            {{
              $t("pa06.execution.updated", {
                time: fmtTime(detail?.task.updated_at ?? null)
              })
            }}
          </p>
          <Select
            :model-value="selectedTaskId"
            :options="taskOptions"
            :aria-label="$t('pa06.execution.pickTask')"
            data-testid="ex-task-picker"
            @update:model-value="pickTask"
          />
        </div>

        <!-- 执行计划：PA-06 状态机链（真实状态，不编造步骤步数） -->
        <section class="ex-card" data-testid="ex-plan">
          <span class="ex-eyebrow">{{ $t("pa06.execution.planEyebrow") }}</span>
          <div class="ex-chain">
            <template v-for="(state, index) in STATE_CHAIN" :key="state">
              <span
                class="ex-chain__state"
                :class="{
                  'is-done': chainPosition > index,
                  'is-current': chainPosition === index
                }"
              >
                {{ $t(`pa06.executionState.${state}`) }}
              </span>
              <span
                v-if="index < STATE_CHAIN.length - 1"
                class="ex-chain__arrow"
                >→</span
              >
            </template>
          </div>
          <p v-if="latestHistory" class="ex-plan__now">
            {{
              $t("pa06.execution.nowLine", {
                action: latestHistory.action,
                time: fmtTime(latestHistory.created_at)
              })
            }}
          </p>
          <Badge variant="neutral">{{
            $t("pa06.execution.chainLegend")
          }}</Badge>
        </section>

        <!-- 计划与依赖 -->
        <span class="ex-eyebrow">{{ $t("pa06.execution.depsEyebrow") }}</span>
        <div class="ex-two-col">
          <div class="ex-card" data-testid="ex-decomposition">
            <h3 class="ex-card__title">
              {{ $t("pa06.execution.decompositionTitle") }}
            </h3>
            <div
              v-for="dep in dependencies"
              :key="dep.id"
              class="ex-dep"
              :data-testid="`ex-dep-${dep.direction}`"
            >
              <span class="ex-dep__marker">
                {{
                  dep.status === "completed"
                    ? "✓"
                    : dep.direction === "upstream"
                      ? "←"
                      : "→"
                }}
              </span>
              <div>
                <p class="ex-dep__title">{{ dep.title }}</p>
                <p class="ex-dep__meta">
                  {{ dep.status }} · {{ dep.direction }}
                </p>
              </div>
            </div>
            <p v-if="dependencies.length === 0" class="ex-dep__meta">
              {{ $t("pa06.execution.noDependencies") }}
            </p>
          </div>
          <div class="ex-card" data-testid="ex-artifacts">
            <h3 class="ex-card__title">
              {{ $t("pa06.execution.artifactsTitle") }}
            </h3>
            <ul class="ex-artifacts">
              <li v-for="line in deliverableLines" :key="line">{{ line }}</li>
            </ul>
          </div>
        </div>

        <!-- ACTIVITY · PROJECT RECORD -->
        <span class="ex-eyebrow">{{
          $t("pa06.execution.activityEyebrow")
        }}</span>
        <div class="ex-activity" data-testid="ex-activity">
          <p
            v-for="entry in historyLines"
            :key="entry.id"
            class="ex-activity__line"
          >
            <span class="ex-activity__time">{{
              fmtTime(entry.created_at)
            }}</span>
            <span>{{ entry.action }}</span>
          </p>
          <p v-if="historyLines.length === 0" class="ex-dep__meta">
            {{ $t("pa06.execution.noActivity") }}
          </p>
        </div>
        <div class="ex-actions">
          <Button variant="primary" size="sm" @click="openVerification">
            {{ $t("pa06.execution.openVerification") }}
          </Button>
          <Button variant="secondary" size="sm" @click="inspectRecovery">
            {{ $t("pa06.execution.inspectRecovery") }}
          </Button>
        </div>

        <template #context>
          <CollabContextPanel
            :title="$t('pa06.execution.ctxTitle')"
            :badge="$t('pa06.execution.ctxBadge')"
          >
            <CollabContextSection :label="$t('pa06.execution.ctxObjective')">
              <template v-if="tracesText">{{ tracesText }}</template>
              <template v-else>{{ goalLine }}</template>
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.execution.ctxChanged')">
              {{ $t("pa06.execution.ctxChangedText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.execution.ctxSafe')">
              {{ $t("pa06.execution.ctxSafeText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.execution.ctxControl')">
              {{ $t("pa06.execution.ctxControlText") }}
            </CollabContextSection>
            <p class="ex-ctx-note">{{ $t("pa06.execution.pauseGapNote") }}</p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.ex-eyebrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.ex-head {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
}

.ex-head__eyebrow-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: var(--space-3);
}

.ex-head__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: 1.375rem;
  font-weight: var(--weight-semibold);
  letter-spacing: -0.018em;
  color: var(--text-primary);
}

.ex-head__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.ex-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.ex-card__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ex-chain {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.ex-chain__state {
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.ex-chain__state.is-done {
  color: var(--text-primary);
}

.ex-chain__state.is-current {
  color: var(--ws-accent);
}

.ex-chain__arrow {
  color: var(--text-secondary);
}

.ex-plan__now {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.ex-two-col {
  display: flex;
  align-items: stretch;
  gap: var(--space-4);
}

.ex-two-col > .ex-card {
  flex: 1;
  min-width: 0;
}

.ex-dep {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
}

.ex-dep__marker {
  color: var(--text-secondary);
  font-family: var(--font-ui);
}

.ex-dep__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ex-dep__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.ex-artifacts {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.ex-activity {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ex-activity__line {
  margin: 0;
  display: flex;
  gap: var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.ex-activity__time {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.ex-actions {
  display: flex;
  gap: var(--space-2);
}

.ex-ctx-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: 1.45;
  color: var(--text-secondary);
}

@media (max-width: 900px) {
  .ex-two-col {
    flex-direction: column;
  }
}
</style>
