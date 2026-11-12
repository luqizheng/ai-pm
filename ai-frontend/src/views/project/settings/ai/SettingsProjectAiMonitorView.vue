<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import {
  getProjectAiTeamMonitor,
  type AiTeamEventRow,
  type AiTeamRunRow
} from "@/api/aiTeam";
import AiTeamFrame from "./AiTeamFrame.vue";
import { useProjectSettingsFrame } from "../shared";

/**
 * Project Setting · AI Team · Execution Monitor 子页（UI-2）。
 *
 * 数据 = `GET /ai-team/monitor` 聚合（pm-api `ai_team_monitor.rs`，U-1=C：
 * 现有表只读聚合，不建事件表——事件表挂 Phase 4）：
 * - Runs 表：task / status / stage / attempt / executor / release_reason /
 *   shield / 时间（首屏后端固定 50 条）；
 * - Events 时间线：action / actor / reason / 时间（首屏后端固定 100 条）。
 * V1 不做分页（`before`/limit 参数挂后续批次，首屏 50/100 足够）；
 * 也不做自动轮询——刷新按钮手动拉取（监控页是排障入口，非实时面板）。
 */
const { t } = useI18n();
const settingsFrame = useProjectSettingsFrame();
const projectId = settingsFrame.projectId;

const runs = ref<AiTeamRunRow[]>([]);
const events = ref<AiTeamEventRow[]>([]);
const loading = ref(true);
const errorMessage = ref("");

async function load() {
  if (!projectId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  errorMessage.value = "";
  try {
    const data = await getProjectAiTeamMonitor(projectId.value);
    runs.value = data.runs;
    events.value = data.events;
  } catch (error) {
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

/** Run 状态 → Badge 语义色（未知状态回退 neutral） */
function runStatusTone(
  status: string
): "success" | "danger" | "info" | "neutral" {
  if (status === "succeeded") return "success";
  if (status === "failed" || status === "timed_out") return "danger";
  if (status === "running" || status === "claimed") return "info";
  return "neutral";
}

/** 调度保护 ≠ none 时给出警示色 */
function shieldTone(shield: string): "warning" | "neutral" {
  return shield && shield !== "none" ? "warning" : "neutral";
}

/** actor 摘要：审计行缺失（理论不应发生）回退「系统」 */
function actorName(event: AiTeamEventRow): string {
  return event.actor ?? t("aiTeam.monitor.systemActor");
}

function formatTime(value: string | null): string {
  return value ? new Date(value).toLocaleString() : "—";
}

const runsDetail = computed(() =>
  t("aiTeam.monitor.runsDetail", { count: runs.value.length })
);
const eventsDetail = computed(() =>
  t("aiTeam.monitor.eventsDetail", { count: events.value.length })
);

onMounted(load);
</script>

<template>
  <AiTeamFrame :description="t('aiTeam.monitor.description')">
    <template #actions>
      <Button
        variant="ghost"
        size="sm"
        :disabled="loading"
        data-testid="ai-monitor-refresh"
        @click="load"
      >
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("aiTeam.monitor.refresh") }}
      </Button>
    </template>

    <div class="ai-monitor">
      <Alert
        v-if="errorMessage"
        variant="danger"
        data-testid="ai-monitor-error"
      >
        {{ t("aiTeam.monitor.loadFailed") }}: {{ errorMessage }}
      </Alert>

      <!-- 无项目上下文：不发请求，指引从总控台进入（scope 哨兵不做项目 id） -->
      <EmptyState
        v-if="!projectId"
        :title="t('aiTeam.noProject.title')"
        :description="t('aiTeam.noProject.hint')"
        data-testid="ai-monitor-no-project"
      />

      <template v-else>
        <!-- Runs 表 -->
        <section
          class="ai-monitor__runs"
          data-testid="ai-monitor-runs"
          :aria-busy="loading"
        >
          <header class="ai-monitor__head">
            <h3>{{ t("aiTeam.monitor.runsHeading") }}</h3>
            <span class="ai-monitor__head-detail">{{ runsDetail }}</span>
          </header>

          <EmptyState
            v-if="!loading && runs.length === 0"
            :title="
              errorMessage
                ? t('aiTeam.monitor.loadFailed')
                : t('aiTeam.monitor.runsEmpty')
            "
            data-testid="ai-monitor-runs-empty"
          />

          <template v-else>
            <span
              class="ai-monitor__grid ai-monitor__head-row"
              aria-hidden="true"
            >
              <span>{{ t("aiTeam.monitor.column.task") }}</span>
              <span>{{ t("aiTeam.monitor.column.status") }}</span>
              <span>{{ t("aiTeam.monitor.column.stage") }}</span>
              <span>{{ t("aiTeam.monitor.column.attempt") }}</span>
              <span>{{ t("aiTeam.monitor.column.executor") }}</span>
              <span>{{ t("aiTeam.monitor.column.releaseReason") }}</span>
              <span>{{ t("aiTeam.monitor.column.shield") }}</span>
              <span>{{ t("aiTeam.monitor.column.time") }}</span>
            </span>
            <div
              v-for="run in runs"
              :key="run.run_id"
              class="ai-monitor__grid ai-monitor__row"
              :data-testid="`ai-monitor-run-${run.run_id}`"
            >
              <span class="ai-monitor__task" :title="run.task_title">
                {{ run.task_title }}
              </span>
              <span>
                <Badge :variant="runStatusTone(run.status)" size="sm">
                  {{ run.status }}
                </Badge>
              </span>
              <span class="ai-monitor__cell">{{ run.stage ?? "—" }}</span>
              <span class="ai-monitor__cell">#{{ run.attempt_no }}</span>
              <span class="ai-monitor__cell">{{
                run.executor_kind ?? "—"
              }}</span>
              <span class="ai-monitor__cell">{{
                run.release_reason ?? "—"
              }}</span>
              <span>
                <Badge :variant="shieldTone(run.scheduling_shield)" size="sm">
                  {{ run.scheduling_shield }}
                </Badge>
              </span>
              <span class="ai-monitor__cell ai-monitor__time">
                {{ formatTime(run.created_at) }}
              </span>
            </div>
          </template>
        </section>

        <!-- Events 时间线 -->
        <section
          class="ai-monitor__events"
          data-testid="ai-monitor-events"
          :aria-busy="loading"
        >
          <header class="ai-monitor__head">
            <h3>{{ t("aiTeam.monitor.eventsHeading") }}</h3>
            <span class="ai-monitor__head-detail">{{ eventsDetail }}</span>
          </header>

          <EmptyState
            v-if="!loading && events.length === 0"
            :title="
              errorMessage
                ? t('aiTeam.monitor.loadFailed')
                : t('aiTeam.monitor.eventsEmpty')
            "
            data-testid="ai-monitor-events-empty"
          />

          <ol v-else class="ai-monitor__timeline">
            <li
              v-for="(event, index) in events"
              :key="`${event.target_id}-${event.created_at}-${index}`"
              class="ai-monitor__event"
              :data-testid="`ai-monitor-event-${index}`"
            >
              <span class="ai-monitor__event-dot" aria-hidden="true" />
              <div class="ai-monitor__event-body">
                <div class="ai-monitor__event-line">
                  <code class="ai-monitor__event-action">{{
                    event.action
                  }}</code>
                  <span class="ai-monitor__event-actor">{{
                    actorName(event)
                  }}</span>
                  <span class="ai-monitor__event-time">
                    {{ formatTime(event.created_at) }}
                  </span>
                </div>
                <p v-if="event.reason" class="ai-monitor__event-reason">
                  {{ event.reason }}
                </p>
                <p v-if="event.target_title" class="ai-monitor__event-target">
                  {{ event.target_title }}
                </p>
              </div>
            </li>
          </ol>
        </section>
      </template>
    </div>
  </AiTeamFrame>
</template>

<style scoped>
.ai-monitor {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.ai-monitor__runs,
.ai-monitor__events {
  display: flex;
  flex-direction: column;
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow-x: auto;
}

.ai-monitor__head {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-monitor__head h3 {
  margin: 0;
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
}

.ai-monitor__head-detail {
  overflow: hidden;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Runs 表：8 列固定宽 + 横向滚动兜底（照 Roles 子页表口径） */
.ai-monitor__grid {
  display: grid;
  grid-template-columns:
    minmax(180px, 1fr)
    110px 90px 70px 100px 150px 110px 170px;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-4);
  min-width: 900px;
}

.ai-monitor__head-row {
  height: var(--size-row);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-monitor__row {
  min-height: var(--size-row);
  padding-top: var(--space-2);
  padding-bottom: var(--space-2);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-monitor__row:last-child {
  border-bottom: none;
}

.ai-monitor__task {
  overflow: hidden;
  min-width: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-monitor__cell {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-monitor__time {
  text-align: right;
}

/* Events 时间线：左缘点线 + 事件卡 */
.ai-monitor__timeline {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: var(--space-3) var(--space-4);
  list-style: none;
}

.ai-monitor__event {
  position: relative;
  display: flex;
  gap: var(--space-3);
  padding-left: var(--space-2);
}

/* 时间线竖线（除最后一项）；几何 = token + ≤2px hairline 微调（prism 规则 3） */
.ai-monitor__event:not(:last-child)::before {
  position: absolute;
  top: calc(var(--space-3) + 2px);
  bottom: calc(-1 * (var(--space-3) + 2px));
  left: calc(var(--size-indicator) + 1px);
  width: var(--border-1);
  background: var(--ws-border);
  content: "";
}

.ai-monitor__event-dot {
  flex: none;
  width: var(--size-timeline-dot);
  height: var(--size-timeline-dot);
  margin-top: var(--size-dot-xs);
  border-radius: 50%;
  background: var(--ws-text-secondary);
}

.ai-monitor__event-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.ai-monitor__event-line {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  min-width: 0;
}

.ai-monitor__event-action {
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-monitor__event-actor {
  flex: none;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-monitor__event-time {
  overflow: hidden;
  margin-left: auto;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-monitor__event-reason {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-monitor__event-target {
  overflow: hidden;
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
