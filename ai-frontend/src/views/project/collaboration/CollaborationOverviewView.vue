<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { translateError } from "@/api/base";
import { listTasks } from "@/api/pm";
import type { Task } from "@/api/types";
import { fetchProjectFeed } from "@/api/feeds";
import { listAgents } from "@/api/execution-plans";
import { getProjectGoal } from "@/api/goal";
import {
  fetchDecisionCenter,
  fetchProjectRisks,
  type DecisionQueueItem,
  type RiskRow
} from "@/api/collaboration";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import AttentionBadge from "./components/AttentionBadge.vue";
import ExecutionStateBadge from "./components/ExecutionStateBadge.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 A — Project collaboration（Figma 259:58，Overview 页签）。
 *
 * 结构化四段：YOUR ATTENTION（待决摘要+入口）/ AUTONOMOUS EXECUTION
 * （任务×agent×状态×进度）/ IMPORTANT RISK / RECENT OUTCOMES。
 * 右栏 = AI-PM Project context（CURRENT DIRECTION / AUTHORIZED BOUNDARY /
 * PROGRESS, NOT SUPERVISION）。决策区空态按画布：常规 AI 工作无需批准继续，
 * 已决记录保留历史。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);

const pendingItems = ref<DecisionQueueItem[]>([]);
const pendingCount = ref(0);
const tasks = ref<Task[]>([]);
const agentNames = ref<Map<string, string>>(new Map());
const openRisks = ref<RiskRow[]>([]);
const outcomes = ref<
  Array<{ id: string; summary: string; created_at: string }>
>([]);
const goalLine = ref<string | null>(null);
const goalRevision = ref<number | null>(null);

const OPEN_STATES = new Set([
  "ready",
  "planned",
  "running",
  "reviewing",
  "verifying",
  "blocked",
  "waiting"
]);

const openTasks = computed(() =>
  tasks.value.filter(task => OPEN_STATES.has(task.status))
);

const taskCounts = computed(() => {
  const total = tasks.value.length;
  const completed = tasks.value.filter(
    task => task.status === "completed"
  ).length;
  const executing = tasks.value.filter(
    task => task.status === "running" || task.status === "ready"
  ).length;
  const verifying = tasks.value.filter(
    task => task.status === "verifying" || task.status === "reviewing"
  ).length;
  const waiting = tasks.value.filter(
    task => task.status === "blocked" || task.status === "waiting"
  ).length;
  const planned = tasks.value.filter(
    task => task.status === "planned" || task.status === "draft"
  ).length;
  return { total, completed, executing, verifying, waiting, planned };
});

function agentName(task: Task): string {
  if (!task.assignee_id) return t("pa06.overview.unassigned");
  return (
    agentNames.value.get(task.assignee_id) ?? t("pa06.overview.agentFallback")
  );
}

function taskProgress(task: Task): string {
  if (task.deps_total > 0) {
    return t("pa06.overview.progressDeps", {
      done: task.deps_met,
      total: task.deps_total
    });
  }
  return "—";
}

function riskMeta(risk: RiskRow): string {
  return `${risk.impact} · ${risk.probability}`;
}

function outcomeTime(iso: string): string {
  return new Date(iso).toLocaleTimeString();
}

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    const id = projectId.value;
    const [center, taskRows, risks, feed, roster, goal] = await Promise.all([
      fetchDecisionCenter(id),
      listTasks({ project_id: id, page: 1, size: 200 }),
      fetchProjectRisks(id),
      fetchProjectFeed(id, 5),
      listAgents(id).catch(() => []),
      getProjectGoal(id).catch(() => null)
    ]);
    pendingItems.value = center.items.filter(item => item.status === "pending");
    pendingCount.value = center.pendingCount;
    tasks.value = taskRows.items ?? [];
    agentNames.value = new Map(
      roster.map((row): [string, string] => [row.id, row.name])
    );
    openRisks.value = risks.filter(risk => risk.status === "open").slice(0, 3);
    outcomes.value = feed.map(row => ({
      id: row.id,
      summary: row.summary,
      created_at: row.created_at
    }));
    if (goal && goal.state === "ready" && goal.markdown) {
      goalLine.value = goal.markdown.split("\n").find(l => l.trim()) ?? null;
      goalRevision.value = goal.metadata?.revision_no ?? null;
    } else {
      goalLine.value = null;
      goalRevision.value = null;
    }
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

function reviewDecision(): void {
  void router.push("/project/collaboration/decisions");
}

function inspectAuthorization(): void {
  void router.push("/project/collaboration/authorization");
}

onMounted(() => {
  if (hasProject.value) void load();
});
watch(projectId, () => {
  if (hasProject.value) void load();
});
</script>

<template>
  <CollaborationFrame>
    <ScreenFeedback
      v-if="loading || error"
      :state="error ? 'error' : 'loading'"
      :title="error ? $t('pa06.feedback.errorTitle') : undefined"
      @retry="load"
    />
    <template v-else>
      <CollabLayout>
        <template v-if="!hasProject">
          <ScreenFeedback state="empty" :title="$t('pa06.frame.noProject')" />
        </template>
        <template v-else>
          <!-- YOUR ATTENTION -->
          <section data-testid="collab-attention">
            <div class="ov-eyebrow-row">
              <span class="ov-eyebrow">{{
                $t("pa06.overview.attention")
              }}</span>
              <AttentionBadge
                v-if="pendingCount > 0"
                level="human_decision"
                :label="
                  $t('pa06.overview.decisionsBadge', { count: pendingCount })
                "
              />
            </div>
            <ScreenFeedback
              v-if="pendingCount === 0"
              state="empty"
              :title="$t('pa06.overview.noDecisions')"
              :description="$t('pa06.overview.noDecisionsHint')"
            />
            <div v-else class="ov-card">
              <p class="ov-card__headline">
                {{ $t("pa06.overview.attentionHeadline") }}
              </p>
              <div
                v-for="item in pendingItems.slice(0, 2)"
                :key="item.id"
                class="ov-decision"
                :data-testid="`ov-decision-${item.kind}`"
              >
                <div class="ov-decision__body">
                  <p class="ov-decision__title">{{ item.title }}</p>
                  <p class="ov-decision__meta">
                    {{ item.kindLabel }} · {{ item.meta }}
                  </p>
                </div>
                <Button variant="primary" size="sm" @click="reviewDecision">
                  {{ $t("pa06.overview.reviewChange") }}
                </Button>
              </div>
              <button
                v-if="pendingCount > 2"
                type="button"
                class="ov-more"
                @click="reviewDecision"
              >
                {{
                  $t("pa06.overview.moreDecisions", { count: pendingCount - 2 })
                }}
              </button>
            </div>
          </section>

          <!-- AUTONOMOUS EXECUTION -->
          <section data-testid="collab-execution">
            <div class="ov-eyebrow-row">
              <span class="ov-eyebrow">{{
                $t("pa06.overview.execution")
              }}</span>
              <AttentionBadge
                level="informational"
                :label="$t('pa06.overview.coordinating')"
              />
            </div>
            <ScreenFeedback
              v-if="openTasks.length === 0"
              state="empty"
              :title="$t('pa06.overview.noOpenTasks')"
              :description="$t('pa06.overview.noOpenTasksHint')"
            />
            <div v-else class="ov-card">
              <div class="ov-exec__head">
                <span>{{ $t("pa06.overview.colTask") }}</span>
                <span>{{ $t("pa06.overview.colStatus") }}</span>
                <span>{{ $t("pa06.overview.colProgress") }}</span>
              </div>
              <div
                v-for="task in openTasks.slice(0, 6)"
                :key="task.id"
                class="ov-exec__row"
                :data-testid="`ov-task-${task.code}`"
              >
                <div class="ov-exec__body">
                  <p class="ov-exec__title">
                    {{ task.code }} · {{ task.title }}
                  </p>
                  <p class="ov-exec__meta">
                    {{ agentName(task) }} ·
                    {{ $t("pa06.overview.coordinatedBy") }}
                  </p>
                </div>
                <ExecutionStateBadge :status="task.status" />
                <span class="ov-exec__progress">{{ taskProgress(task) }}</span>
              </div>
            </div>
          </section>

          <!-- IMPORTANT RISK -->
          <section v-if="openRisks.length > 0" data-testid="collab-risk">
            <span class="ov-eyebrow">{{ $t("pa06.overview.risk") }}</span>
            <div
              v-for="risk in openRisks"
              :key="risk.id"
              class="ov-risk"
              data-testid="ov-risk"
            >
              <p class="ov-risk__title">{{ risk.title }}</p>
              <p class="ov-risk__meta">
                {{ riskMeta(risk) }} ·
                {{ $t("pa06.overview.riskOpen") }}
              </p>
            </div>
          </section>

          <!-- RECENT OUTCOMES -->
          <section data-testid="collab-outcomes">
            <span class="ov-eyebrow">{{ $t("pa06.overview.outcomes") }}</span>
            <ScreenFeedback
              v-if="outcomes.length === 0"
              state="empty"
              :title="$t('pa06.overview.noOutcomes')"
            />
            <ul v-else class="ov-outcomes">
              <li
                v-for="row in outcomes"
                :key="row.id"
                class="ov-outcomes__line"
              >
                <span class="ov-outcomes__time">{{
                  outcomeTime(row.created_at)
                }}</span>
                <span>{{ row.summary }}</span>
              </li>
            </ul>
          </section>
        </template>

        <!-- 右栏：AI-PM · Project context -->
        <template #context>
          <CollabContextPanel
            :title="$t('pa06.overview.ctxTitle')"
            :badge="$t('pa06.overview.ctxScope')"
          >
            <CollabContextSection :label="$t('pa06.overview.ctxDirection')">
              <template v-if="goalLine">{{ goalLine }}</template>
              <template v-else>{{ $t("pa06.overview.ctxNoGoal") }}</template>
            </CollabContextSection>
            <p v-if="goalRevision" class="ctx-meta">
              {{ $t("pa06.overview.ctxDirectionMeta", { rev: goalRevision }) }}
            </p>
            <CollabContextSection :label="$t('pa06.overview.ctxBoundary')">
              {{ $t("pa06.overview.ctxBoundaryText") }}
            </CollabContextSection>
            <Button variant="secondary" size="sm" @click="inspectAuthorization">
              {{ $t("pa06.overview.ctxInspect") }}
            </Button>
            <CollabContextSection :label="$t('pa06.overview.ctxProgress')">
              {{
                $t("pa06.overview.ctxProgressText", {
                  total: taskCounts.total,
                  completed: taskCounts.completed,
                  executing: taskCounts.executing,
                  verifying: taskCounts.verifying,
                  waiting: taskCounts.waiting,
                  planned: taskCounts.planned
                })
              }}
            </CollabContextSection>
            <p class="ctx-meta">{{ $t("pa06.overview.ctxNote") }}</p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.ov-eyebrow-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.ov-eyebrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.ov-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-top: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.ov-card__headline {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ov-decision {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.ov-decision__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ov-decision__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.ov-more {
  align-self: flex-start;
  border: none;
  background: transparent;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-accent);
  cursor: pointer;
}

.ov-exec__head,
.ov-exec__row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 8rem 8rem;
  gap: var(--space-3);
  align-items: center;
}

.ov-exec__head {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.ov-exec__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ov-exec__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.ov-exec__progress {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.ov-risk {
  margin-top: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-3) var(--space-4);
}

.ov-risk__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-color-danger-text);
}

.ov-risk__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.ov-outcomes {
  margin: var(--space-2) 0 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ov-outcomes__line {
  display: flex;
  gap: var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.ov-outcomes__time {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.ctx-meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: 1.45;
  color: var(--text-secondary);
  white-space: pre-line;
}
</style>
