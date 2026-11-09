<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Plus, RefreshCw, Sparkles } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import { listTasks } from "@/api/tasks";
import type { Task } from "@/api/types";
import { useScopeStore } from "@/api/scope";
import TasksFrame from "../TasksFrame.vue";
import TaskPriorityChip from "../components/TaskPriorityChip.vue";
import TaskStatusChip from "../components/TaskStatusChip.vue";
import {
  TASK_PRIORITIES,
  TASK_STATUSES,
  formatDateTime,
  shortId,
  toTaskRow,
  type TaskRowModel
} from "../tasks-contract";

/**
 * 任务总览页根（S04-UI-01，2026-11-10 落地）。
 *
 * 数据 = pm-api REST（/v1/pm/tasks，size 上限 200）：统计卡、待处理事项与
 * 分布均由前端聚合派生；「AI 规划建议」接口尚未落地——发布态渲染空态，
 * dev（VITE_USE_MOCK=1）动态引入本目录 fixtures 做演示（release-guard
 * 保证演示数据不进 dist）。
 *
 * 规划准备度 ≠ 工程执行进度：卡片语义只覆盖 S04 规划态（draft/ready/
 * blocked 等），执行通道状态（claimed/running/…）不计入规划指标。
 */
const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const scope = useScopeStore();

const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  if (scope.currentProjectId && scope.currentProjectId !== "all") {
    return scope.currentProjectId;
  }
  return "";
});

const tasks = ref<TaskRowModel[]>([]);
const total = ref(0);
const truncated = ref(false);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  loading.value = true;
  loadError.value = false;
  try {
    const result = await listTasks({
      project_id: projectId.value || undefined,
      page: 1,
      size: 200
    });
    tasks.value = (result.items ?? []).map(toTaskRow);
    total.value = result.total ?? tasks.value.length;
    truncated.value = total.value > tasks.value.length;
  } catch {
    loadError.value = true;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

function goBacklog(query: Record<string, string>) {
  void router.push({ path: "/project/tasks/backlog", query });
}

function openDetail(task: TaskRowModel) {
  void router.push(`/project/tasks/${task.id}`);
}

function goCreate() {
  void router.push({ path: "/project/tasks/backlog", query: { create: "1" } });
}

const stats = computed(() => {
  const counts = { total: 0, draft: 0, ready: 0, blocked: 0, unassigned: 0 };
  counts.total = tasks.value.length;
  for (const task of tasks.value) {
    if (task.status === "draft") counts.draft += 1;
    if (task.status === "ready") counts.ready += 1;
    if (task.status === "blocked") counts.blocked += 1;
    if (!task.assigneeId) counts.unassigned += 1;
  }
  return counts;
});

const statusCounts = computed(() =>
  TASK_STATUSES.map(meta => ({
    ...meta,
    count: tasks.value.filter(task => task.status === meta.value).length
  })).filter(item => item.count > 0)
);

const priorityCounts = computed(() =>
  TASK_PRIORITIES.map(value => ({
    value,
    count: tasks.value.filter(task => task.priority === value).length
  })).filter(item => item.count > 0)
);

/** 待人工处理事项：全部从真实列表派生（无伪数据） */
const attention = computed(() => {
  const items: Array<{
    key: string;
    count: number;
    query: Record<string, string>;
  }> = [];
  const drafts = tasks.value.filter(task => task.status === "draft").length;
  const blocked = tasks.value.filter(task => task.status === "blocked").length;
  const openUnassigned = tasks.value.filter(
    task =>
      !task.assigneeId &&
      !["completed", "cancelled", "superseded", "failed"].includes(task.status)
  ).length;
  if (drafts > 0) {
    items.push({ key: "draft", count: drafts, query: { status: "draft" } });
  }
  if (blocked > 0) {
    items.push({
      key: "blocked",
      count: blocked,
      query: { status: "blocked" }
    });
  }
  if (openUnassigned > 0) {
    items.push({
      key: "unassigned",
      count: openUnassigned,
      query: { unassigned: "1" }
    });
  }
  return items;
});

const upcoming = computed(() =>
  [...tasks.value]
    .filter(
      task => !["completed", "cancelled", "superseded"].includes(task.status)
    )
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    .slice(0, 8)
);

/** AI 规划建议：接口未接线，发布态恒为空；dev 动态引入 fixtures 演示 */
const INSIGHTS_MOCK = import.meta.env.VITE_USE_MOCK === "1";

interface InsightItem {
  id: string;
  title: string;
  detail: string;
  source: string;
  impact: string;
}

const insights = ref<InsightItem[]>([]);
const insightDecisions = ref<Record<string, "accepted" | "rejected">>({});
const insightsLoading = ref(false);

async function loadInsights() {
  if (!INSIGHTS_MOCK) return;
  insightsLoading.value = true;
  try {
    const { overviewInsights } = await import("./fixtures");
    insights.value = overviewInsights();
  } finally {
    insightsLoading.value = false;
  }
}

onMounted(loadInsights);

function decideInsight(id: string, decision: "accepted" | "rejected") {
  insightDecisions.value = { ...insightDecisions.value, [id]: decision };
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('shell.app.side.item.taskManagement') }]"
    :title="t('s04.tasks.page.overviewTitle')"
    :description="t('s04.tasks.page.overviewDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="tasks-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s04.tasks.common.refresh") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        data-testid="tasks-create"
        @click="goCreate"
      >
        <Plus :size="14" />
        {{ t("s04.tasks.common.createTask") }}
      </Button>
    </template>

    <TasksFrame />

    <Alert
      v-if="loadError"
      variant="danger"
      :title="t('s04.tasks.common.errorTitle')"
    >
      {{ t("s04.tasks.overview.loadFailed") }}
    </Alert>

    <div v-else-if="loading" class="ov-loading">
      {{ t("s04.tasks.common.loading") }}
    </div>

    <template v-else>
      <p v-if="truncated" class="ov-truncated">
        {{ t("s04.tasks.overview.truncatedHint") }}
      </p>

      <section class="ov-stats">
        <button
          v-for="stat in [
            { key: 'total', value: stats.total, query: {} },
            { key: 'draft', value: stats.draft, query: { status: 'draft' } },
            { key: 'ready', value: stats.ready, query: { status: 'ready' } },
            {
              key: 'blocked',
              value: stats.blocked,
              query: { status: 'blocked' }
            },
            {
              key: 'unassigned',
              value: stats.unassigned,
              query: { unassigned: '1' }
            }
          ]"
          :key="stat.key"
          type="button"
          class="ov-stat"
          :data-testid="`ov-stat-${stat.key}`"
          @click="goBacklog(stat.query)"
        >
          <span class="ov-stat__value">{{ stat.value }}</span>
          <span class="ov-stat__label">{{
            t(
              `s04.tasks.overview.stat${stat.key.charAt(0).toUpperCase()}${stat.key.slice(1)}`
            )
          }}</span>
          <span class="ov-stat__hint">{{
            t(
              `s04.tasks.overview.statHint${stat.key.charAt(0).toUpperCase()}${stat.key.slice(1)}`
            )
          }}</span>
        </button>
      </section>

      <section class="ov-card">
        <header class="ov-card__head">
          <h2 class="ov-card__title">
            {{ t("s04.tasks.overview.attentionTitle") }}
          </h2>
        </header>
        <EmptyState
          v-if="attention.length === 0"
          :title="t('s04.tasks.overview.attentionEmpty')"
          size="sm"
        />
        <ul v-else class="ov-attention">
          <li
            v-for="item in attention"
            :key="item.key"
            class="ov-attention__item"
          >
            <span class="ov-attention__text">{{
              t(
                `s04.tasks.overview.attention${item.key.charAt(0).toUpperCase()}${item.key.slice(1)}`,
                { count: item.count }
              )
            }}</span>
            <Button
              variant="secondary"
              size="sm"
              @click="goBacklog(item.query)"
            >
              {{ t("s04.tasks.overview.attentionGo") }}
            </Button>
          </li>
        </ul>
      </section>

      <section class="ov-card">
        <header class="ov-card__head">
          <h2 class="ov-card__title">
            {{ t("s04.tasks.overview.distributionTitle") }}
          </h2>
        </header>
        <div class="ov-dist">
          <div class="ov-dist__group">
            <h3 class="ov-dist__label">
              {{ t("s04.tasks.overview.distributionStatus") }}
            </h3>
            <div v-if="statusCounts.length === 0" class="ov-dist__empty">
              {{ t("s04.tasks.common.empty") }}
            </div>
            <div v-else class="ov-dist__bar">
              <button
                v-for="item in statusCounts"
                :key="item.value"
                type="button"
                class="ov-dist__segment"
                :class="`ov-dist__segment--${item.tone}`"
                :style="{ flexGrow: item.count }"
                :title="`${t(`s04.tasks.status.${item.value}`)} · ${item.count}`"
                @click="goBacklog({ status: item.value })"
              />
            </div>
            <div v-if="statusCounts.length > 0" class="ov-dist__legend">
              <button
                v-for="item in statusCounts"
                :key="item.value"
                type="button"
                class="ov-dist__legend-item"
                @click="goBacklog({ status: item.value })"
              >
                <TaskStatusChip :status="item.value" />
                <span class="ov-dist__count">{{ item.count }}</span>
              </button>
            </div>
          </div>
          <div class="ov-dist__group">
            <h3 class="ov-dist__label">
              {{ t("s04.tasks.overview.distributionPriority") }}
            </h3>
            <div v-if="priorityCounts.length === 0" class="ov-dist__empty">
              {{ t("s04.tasks.common.empty") }}
            </div>
            <div v-else class="ov-dist__bar">
              <button
                v-for="item in priorityCounts"
                :key="item.value"
                type="button"
                class="ov-dist__segment"
                :class="`ov-dist__segment--${item.value === 'urgent' ? 'danger' : item.value === 'high' ? 'warning' : item.value === 'low' ? 'muted' : 'neutral'}`"
                :style="{ flexGrow: item.count }"
                :title="`${t(`s04.tasks.priority.${item.value}`)} · ${item.count}`"
                @click="goBacklog({ priority: item.value })"
              />
            </div>
            <div v-if="priorityCounts.length > 0" class="ov-dist__legend">
              <button
                v-for="item in priorityCounts"
                :key="item.value"
                type="button"
                class="ov-dist__legend-item"
                @click="goBacklog({ priority: item.value })"
              >
                <TaskPriorityChip :priority="item.value" />
                <span class="ov-dist__count">{{ item.count }}</span>
              </button>
            </div>
          </div>
        </div>
        <p class="ov-note">{{ t("s04.tasks.overview.planningNote") }}</p>
      </section>

      <section class="ov-card">
        <header class="ov-card__head">
          <h2 class="ov-card__title">
            {{ t("s04.tasks.overview.upcomingTitle") }}
          </h2>
          <Button variant="ghost" size="sm" @click="goBacklog({})">
            {{ t("s04.tasks.tabs.backlog") }}
          </Button>
        </header>
        <EmptyState
          v-if="upcoming.length === 0"
          :title="t('s04.tasks.common.empty')"
          :description="t('s04.tasks.common.emptyHint')"
          size="sm"
        />
        <table v-else class="ov-table">
          <thead>
            <tr>
              <th>{{ t("s04.tasks.common.taskLabel") }}</th>
              <th>{{ t("s04.tasks.common.statusLabel") }}</th>
              <th>{{ t("s04.tasks.common.priorityLabel") }}</th>
              <th>{{ t("s04.tasks.common.assigneeLabel") }}</th>
              <th>{{ t("s04.tasks.common.updatedLabel") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="task in upcoming"
              :key="task.id"
              class="ov-table__row"
              @click="openDetail(task)"
            >
              <td>
                <span class="ov-table__id">{{ shortId(task.id) }}</span>
                <span class="ov-table__title">{{ task.title }}</span>
              </td>
              <td><TaskStatusChip :status="task.status" /></td>
              <td><TaskPriorityChip :priority="task.priority" /></td>
              <td>
                {{
                  task.assigneeId
                    ? shortId(task.assigneeId)
                    : t("s04.tasks.common.unassigned")
                }}
              </td>
              <td>{{ formatDateTime(task.updatedAt) }}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section class="ov-card">
        <header class="ov-card__head">
          <h2 class="ov-card__title">
            <Sparkles :size="14" />
            {{ t("s04.tasks.overview.insightsTitle") }}
          </h2>
        </header>
        <p v-if="insightsLoading" class="ov-loading">
          {{ t("s04.tasks.common.loading") }}
        </p>
        <EmptyState
          v-else-if="insights.length === 0"
          :title="t('s04.tasks.overview.insightsEmpty')"
          size="sm"
        />
        <template v-else>
          <p class="ov-note">{{ t("s04.tasks.overview.insightsDevNote") }}</p>
          <ul class="ov-insights">
            <li
              v-for="insight in insights"
              :key="insight.id"
              class="ov-insight"
              :data-testid="`insight-${insight.id}`"
            >
              <p class="ov-insight__title">{{ insight.title }}</p>
              <p class="ov-insight__detail">{{ insight.detail }}</p>
              <p class="ov-insight__meta">
                {{ insight.source }} · {{ insight.impact }}
              </p>
              <div class="ov-insight__actions">
                <template v-if="insightDecisions[insight.id]">
                  <span class="ov-insight__decided">
                    {{
                      insightDecisions[insight.id] === "accepted"
                        ? t("s04.tasks.overview.insightAccepted")
                        : t("s04.tasks.overview.insightRejected")
                    }}
                  </span>
                </template>
                <template v-else>
                  <Button
                    variant="secondary"
                    size="sm"
                    @click="decideInsight(insight.id, 'accepted')"
                  >
                    {{ t("s04.tasks.overview.insightAccept") }}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    @click="decideInsight(insight.id, 'rejected')"
                  >
                    {{ t("s04.tasks.overview.insightReject") }}
                  </Button>
                </template>
              </div>
            </li>
          </ul>
        </template>
      </section>
    </template>
  </PageFrame>
</template>

<style scoped>
.ov-loading {
  padding: var(--space-8) 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  text-align: center;
}

.ov-truncated {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.ov-stats {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: var(--space-3);
}

.ov-stat {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  text-align: left;
  cursor: pointer;
  transition: border-color var(--duration-fast) var(--ease-decelerate);
}

.ov-stat:hover {
  border-color: var(--ws-accent);
}

.ov-stat:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.ov-stat__value {
  font-size: var(--font-size-stat);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
  font-variant-numeric: tabular-nums;
}

.ov-stat__label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.ov-stat__hint {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.ov-card {
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  padding: var(--space-4) var(--space-5);
}

.ov-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-3);
}

.ov-card__title {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.ov-attention {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ov-attention__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.ov-attention__text {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.ov-dist {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-6);
}

.ov-dist__label {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.ov-dist__bar {
  display: flex;
  height: var(--space-3);
  border-radius: var(--radius-xs);
  overflow: hidden;
  background: var(--ws-elevated);
}

.ov-dist__segment {
  min-width: var(--space-2);
  border: none;
  padding: 0;
  cursor: pointer;
}

.ov-dist__segment--neutral {
  background: var(--ws-text-disabled);
}

.ov-dist__segment--info {
  background: var(--ws-color-info);
}

.ov-dist__segment--ready {
  background: var(--ws-color-status-ready);
}

.ov-dist__segment--warning {
  background: var(--ws-color-warning);
}

.ov-dist__segment--danger {
  background: var(--ws-color-danger);
}

.ov-dist__segment--success {
  background: var(--ws-color-success);
}

.ov-dist__segment--muted {
  background: var(--ws-border);
}

.ov-dist__legend {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: var(--space-2);
}

.ov-dist__legend-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0;
  border: none;
  background: none;
  cursor: pointer;
}

.ov-dist__count {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.ov-dist__empty {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.ov-note {
  margin: var(--space-3) 0 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.ov-table {
  width: 100%;
  border-collapse: collapse;
}

.ov-table th {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  text-align: left;
}

.ov-table td {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
}

.ov-table__row {
  cursor: pointer;
}

.ov-table__row:hover {
  background: var(--ws-elevated);
}

.ov-table__id {
  display: inline-block;
  min-width: var(--space-10);
  margin-right: var(--space-3);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono, monospace);
  font-size: var(--font-size-caption);
}

.ov-table__title {
  color: var(--ws-text-primary);
}

.ov-insights {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ov-insight {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  border: var(--border-1) dashed var(--ws-color-info);
  border-radius: var(--radius-sm);
  background: var(--ws-color-accent-surface);
}

.ov-insight__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
}

.ov-insight__detail {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.ov-insight__meta {
  margin: 0;
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
}

.ov-insight__actions {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-1);
}

.ov-insight__decided {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}
</style>
