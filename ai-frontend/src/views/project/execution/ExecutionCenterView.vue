<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import {
  getHumanAttentionQueue,
  listProjectRunEvents,
  listTaskRuns,
  type HumanAttentionQueue,
  type RunEvent,
  type TaskRun
} from "@/api/execution-plans";
import { useDesignProject } from "@/views/project/design/use-design-project";
import ExecutionFrame from "./ExecutionFrame.vue";
import {
  formatDateTime,
  formatDuration,
  runLabel,
  runTone,
  shortId,
  waitingTone
} from "./execution-contract";

/**
 * 执行中心（S05-01，接真）。
 *
 * 数据：需要人工处理队列 + Run 列表 + 项目级活动流。
 *
 * 三层必须视觉可辨（设计不变量 3）：**Task 状态 / Run 状态（含 attempt #）/
 * Agent**。等待原因按**五值**原样呈现，不合并成「等待中」。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const queue = ref<HumanAttentionQueue | null>(null);
const runs = ref<TaskRun[]>([]);
const events = ref<RunEvent[]>([]);
const loading = ref(true);
const loadError = ref(false);

const kindLabels = computed(() => ({
  delivery: t("s05.execution.runKind.delivery"),
  review: t("s05.execution.runKind.review")
}));

async function load() {
  if (!hasProject.value) {
    queue.value = null;
    runs.value = [];
    events.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    const [attention, runList, eventList] = await Promise.all([
      getHumanAttentionQueue(projectId.value),
      listTaskRuns({ project_id: projectId.value, page: 1, size: 100 }),
      listProjectRunEvents(projectId.value, 50)
    ]);
    queue.value = attention;
    runs.value = runList.items;
    events.value = eventList;
  } catch {
    loadError.value = true;
    queue.value = null;
    runs.value = [];
    events.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const waiting = computed(() => queue.value?.waiting ?? []);
const interactions = computed(() => queue.value?.interactions ?? []);
const approvals = computed(() => queue.value?.approvals ?? []);

const attentionCount = computed(
  () =>
    waiting.value.length + interactions.value.length + approvals.value.length
);

/** 正在执行：Run 处于 claimed / running。 */
const activeRuns = computed(() =>
  runs.value.filter(run => run.status === "claimed" || run.status === "running")
);

/** 排队：Run 处于 queued。 */
const queuedRuns = computed(() =>
  runs.value.filter(run => run.status === "queued")
);

/** 最近交付：已成功的 delivery run。 */
const deliveredRuns = computed(() =>
  runs.value
    .filter(run => run.run_kind === "delivery" && run.status === "succeeded")
    .slice(0, 10)
);

function openRun(runId: string) {
  void router.push(`/project/execution/runs/${runId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s05.execution.module') }]"
    :title="t('s05.execution.page.centerTitle')"
    :description="t('s05.execution.page.centerDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="ec-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s05.execution.common.refresh") }}
      </Button>
    </template>

    <ExecutionFrame>
      <Alert
        v-if="loadError"
        variant="danger"
        size="sm"
        :title="t('s05.execution.common.errorTitle')"
      />

      <EmptyState
        v-if="!hasProject"
        :title="t('s05.execution.common.noProjectTitle')"
        :description="t('s05.execution.common.noProjectHint')"
      />

      <template v-else>
        <Card class="ec-section" data-testid="ec-attention">
          <template #header>
            {{
              t("s05.execution.center.attentionTitle", {
                count: attentionCount
              })
            }}
          </template>

          <p v-if="attentionCount === 0" class="ec-empty">
            {{ t("s05.execution.center.attentionEmpty") }}
          </p>

          <template v-else>
            <ul v-if="waiting.length > 0" class="ec-list">
              <li
                v-for="item in waiting"
                :key="item.id"
                class="ec-row"
                :data-testid="`ec-waiting-${item.reason}`"
              >
                <Badge :variant="waitingTone(item.reason)" size="sm">
                  {{
                    t(`s05.execution.waitingReason.${item.reason}`, item.reason)
                  }}
                </Badge>
                <span class="ec-row__title">{{ item.task_title }}</span>
                <span class="ec-row__meta">
                  {{ t("s05.execution.center.since") }}
                  {{ formatDateTime(item.since) }}
                </span>
              </li>
            </ul>

            <ul v-if="interactions.length > 0" class="ec-list">
              <li
                v-for="item in interactions"
                :key="item.id"
                class="ec-row"
                :data-testid="`ec-interaction-${item.id}`"
              >
                <Badge variant="warning" size="sm">
                  {{ t("s05.execution.center.pendingInteraction") }}
                </Badge>
                <span class="ec-row__title">{{ item.title }}</span>
                <span class="ec-row__question">{{ item.question }}</span>
                <span class="ec-row__meta">{{
                  formatDateTime(item.deadline_at)
                }}</span>
              </li>
            </ul>

            <ul v-if="approvals.length > 0" class="ec-list">
              <li
                v-for="item in approvals"
                :key="item.id"
                class="ec-row"
                :data-testid="`ec-approval-${item.id}`"
              >
                <Badge variant="warning" size="sm">
                  {{ t("s05.execution.center.pendingApproval") }}
                </Badge>
                <span class="ec-row__title">
                  {{ item.task_title ?? item.kind }}
                </span>
                <span class="ec-row__meta">{{ item.source }}</span>
              </li>
            </ul>
          </template>
        </Card>

        <Card class="ec-section" data-testid="ec-active">
          <template #header>
            {{
              t("s05.execution.center.activeTitle", {
                count: activeRuns.length
              })
            }}
          </template>
          <p v-if="activeRuns.length === 0" class="ec-empty">
            {{ t("s05.execution.center.activeEmpty") }}
          </p>
          <ul v-else class="ec-list">
            <li
              v-for="run in activeRuns"
              :key="run.id"
              class="ec-row"
              :data-testid="`ec-run-${run.id}`"
            >
              <Badge :variant="runTone(run.status)" size="sm">
                {{ t(`s05.execution.runStatus.${run.status}`, run.status) }}
              </Badge>
              <span class="ec-row__title">{{ run.task_title }}</span>
              <span class="ec-row__meta">
                {{ runLabel(run.run_kind, run.attempt_no, kindLabels) }}
              </span>
              <span class="ec-row__meta">
                {{
                  run.agent_id
                    ? shortId(run.agent_id)
                    : t("s05.execution.common.noAgent")
                }}
              </span>
              <Button
                variant="ghost"
                size="sm"
                :data-testid="`ec-open-${run.id}`"
                @click="openRun(run.id)"
              >
                {{ t("s05.execution.common.open") }}
              </Button>
            </li>
          </ul>
        </Card>

        <Card class="ec-section" data-testid="ec-queue">
          <template #header>
            {{
              t("s05.execution.center.queueTitle", { count: queuedRuns.length })
            }}
          </template>
          <p v-if="queuedRuns.length === 0" class="ec-empty">
            {{ t("s05.execution.center.queueEmpty") }}
          </p>
          <ul v-else class="ec-list">
            <li
              v-for="run in queuedRuns"
              :key="run.id"
              class="ec-row"
              :data-testid="`ec-queued-${run.id}`"
            >
              <Badge :variant="runTone(run.status)" size="sm">
                {{ t(`s05.execution.runStatus.${run.status}`, run.status) }}
              </Badge>
              <span class="ec-row__title">{{ run.task_title }}</span>
              <span class="ec-row__meta">
                {{ runLabel(run.run_kind, run.attempt_no, kindLabels) }}
              </span>
            </li>
          </ul>
        </Card>

        <Card class="ec-section" data-testid="ec-delivered">
          <template #header>
            {{
              t("s05.execution.center.deliveredTitle", {
                count: deliveredRuns.length
              })
            }}
          </template>
          <p v-if="deliveredRuns.length === 0" class="ec-empty">
            {{ t("s05.execution.center.deliveredEmpty") }}
          </p>
          <ul v-else class="ec-list">
            <li
              v-for="run in deliveredRuns"
              :key="run.id"
              class="ec-row"
              :data-testid="`ec-delivered-${run.id}`"
            >
              <Badge :variant="runTone(run.status)" size="sm">
                {{ t(`s05.execution.runStatus.${run.status}`, run.status) }}
              </Badge>
              <span class="ec-row__title">{{ run.task_title }}</span>
              <span class="ec-row__meta">
                {{ runLabel(run.run_kind, run.attempt_no, kindLabels) }}
              </span>
              <span class="ec-row__meta">
                {{ formatDuration(run.started_at, run.finished_at) }}
              </span>
              <span class="ec-row__meta">{{
                formatDateTime(run.finished_at)
              }}</span>
            </li>
          </ul>
        </Card>

        <Card class="ec-section" data-testid="ec-events">
          <template #header>{{
            t("s05.execution.center.eventsTitle")
          }}</template>
          <p v-if="events.length === 0" class="ec-empty">
            {{ t("s05.execution.center.eventsEmpty") }}
          </p>
          <ol v-else class="ec-events">
            <li
              v-for="event in events"
              :key="event.id"
              class="ec-event"
              :data-testid="`ec-event-${event.event_type}`"
            >
              <span class="ec-event__time">
                {{ formatDateTime(event.occurred_at) }}
              </span>
              <span class="ec-event__type">
                {{
                  t(
                    `s05.execution.eventType.${event.event_type}`,
                    event.event_type
                  )
                }}
              </span>
              <span class="ec-event__meta">{{ shortId(event.run_id) }}</span>
            </li>
          </ol>
        </Card>

        <p v-if="loading" class="ec-empty">
          {{ t("s05.execution.common.loading") }}
        </p>
      </template>
    </ExecutionFrame>
  </PageFrame>
</template>

<style scoped>
.ec-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ec-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.ec-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ec-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.ec-row__title {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

.ec-row__question {
  flex-basis: 100%;
  color: var(--ws-text-secondary);
}

.ec-row__meta {
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.ec-events {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ec-event {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.ec-event__time {
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.ec-event__type {
  color: var(--ws-text-primary);
}

.ec-event__meta {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
}
</style>
