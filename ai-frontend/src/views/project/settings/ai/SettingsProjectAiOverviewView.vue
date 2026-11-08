<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Bot,
  PauseCircle,
  PlayCircle,
  RefreshCw,
  Server
} from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { StatCard } from "@/components/ui/stat-card";
import {
  getProjectAiTeamMonitor,
  getProjectAiTeamOverview,
  type AiTeamOverview,
  type AiTeamRunRow
} from "@/api/aiTeam";
import AiTeamFrame from "./AiTeamFrame.vue";
import { useProjectSettingsFrame } from "../shared";

/**
 * Project Setting · AI Team · Overview 子页（UI-2）。
 *
 * 数据 = 两聚合端点（pm-api `ai_team_monitor.rs`，U-1=C 只读零迁移）：
 * - `overview`：四组计数（启用角色 / 活跃 Run / 等待或阻塞 / Provider 绑定态）；
 * - `monitor.runs`：最近执行列表复用明细端点的前 {@link RECENT_RUNS} 条，
 *   不另设「概要版 runs」接口（口径单一：与监控页同源同形）。
 * 「查看执行监控」跳 Monitor 子路由（携带 ?project_id= 上下文）。
 */
const RECENT_RUNS = 8;

const { t } = useI18n();
const settingsFrame = useProjectSettingsFrame();
const projectId = settingsFrame.projectId;

const overview = ref<AiTeamOverview | null>(null);
const recentRuns = ref<AiTeamRunRow[]>([]);
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
    const [overviewData, monitorData] = await Promise.all([
      getProjectAiTeamOverview(projectId.value),
      getProjectAiTeamMonitor(projectId.value)
    ]);
    overview.value = overviewData;
    recentRuns.value = monitorData.runs.slice(0, RECENT_RUNS);
  } catch (error) {
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

const statCards = computed(() => {
  const data = overview.value;
  if (!data) return [];
  return [
    {
      key: "roles",
      label: t("aiTeam.overview.stat.rolesEnabled"),
      value: String(data.roles.enabled),
      description: t("aiTeam.overview.stat.rolesDetail", {
        enabled: data.roles.enabled,
        templates: data.roles.templates
      }),
      icon: Bot,
      tone: "accent" as const
    },
    {
      key: "runs",
      label: t("aiTeam.overview.stat.activeRuns"),
      value: String(data.runs.active),
      description: t("aiTeam.overview.stat.succeededDetail", {
        succeeded: data.runs.succeeded_24h,
        failed: data.runs.failed_24h
      }),
      icon: PlayCircle,
      tone: (data.runs.failed_24h > 0 ? "warning" : "info") as
        | "warning"
        | "info"
    },
    {
      key: "tasks",
      label: t("aiTeam.overview.stat.pendingTasks"),
      value: String(data.tasks.awaiting_decision + data.tasks.blocked),
      description: t("aiTeam.overview.stat.pendingTasksDetail", {
        awaiting: data.tasks.awaiting_decision,
        blocked: data.tasks.blocked
      }),
      icon: PauseCircle,
      tone: (data.tasks.awaiting_decision + data.tasks.blocked > 0
        ? "warning"
        : "neutral") as "warning" | "neutral"
    },
    {
      key: "provider",
      label: t("aiTeam.overview.stat.provider"),
      value: data.provider.bound
        ? t("aiTeam.overview.stat.providerBound")
        : t("aiTeam.overview.stat.providerUnbound"),
      description: data.provider.bound
        ? t("aiTeam.overview.stat.providerBoundDetail", {
            name: data.provider.provider_name ?? ""
          })
        : t("aiTeam.overview.stat.providerUnboundDetail"),
      icon: Server,
      tone: (data.provider.bound ? "success" : "neutral") as
        | "success"
        | "neutral"
    }
  ];
});

/** Run 状态 → Badge 语义色（未知状态回退 neutral） */
function runStatusTone(
  status: string
): "success" | "danger" | "info" | "neutral" {
  if (status === "succeeded") return "success";
  if (status === "failed" || status === "timed_out") return "danger";
  if (status === "running" || status === "claimed") return "info";
  return "neutral";
}

function formatTime(value: string | null): string {
  return value ? new Date(value).toLocaleString() : "—";
}

const monitorLink = computed(() =>
  settingsFrame.withProject("/project/settings/ai/monitor")
);

onMounted(load);
</script>

<template>
  <AiTeamFrame :description="t('aiTeam.overview.description')">
    <template #actions>
      <Button
        variant="ghost"
        size="sm"
        :disabled="loading"
        data-testid="ai-overview-refresh"
        @click="load"
      >
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("aiTeam.overview.refresh") }}
      </Button>
    </template>

    <div class="ai-overview">
      <Alert
        v-if="errorMessage"
        variant="danger"
        data-testid="ai-overview-error"
      >
        {{ t("aiTeam.overview.loadFailed") }}: {{ errorMessage }}
      </Alert>

      <!-- 无项目上下文：不发请求，指引从总控台进入（scope 哨兵不做项目 id） -->
      <EmptyState
        v-if="!projectId"
        :title="t('aiTeam.noProject.title')"
        :description="t('aiTeam.noProject.hint')"
        data-testid="ai-overview-no-project"
      />

      <template v-else>
        <div
          class="ai-overview__stats"
          :aria-busy="loading"
          data-testid="ai-overview-stats"
        >
          <StatCard
            v-for="card in statCards"
            :key="card.key"
            :label="card.label"
            :value="card.value"
            :description="card.description"
            :icon="card.icon"
            :tone="card.tone"
            :loading="loading"
            :data-testid="`ai-overview-stat-${card.key}`"
          />
        </div>

        <section
          class="ai-overview__runs"
          data-testid="ai-overview-recent-runs"
          :aria-busy="loading"
        >
          <header class="ai-overview__runs-head">
            <div class="ai-overview__runs-title">
              <h3>{{ t("aiTeam.overview.recentRuns") }}</h3>
              <span class="ai-overview__runs-detail">
                {{
                  t("aiTeam.overview.recentRunsDetail", {
                    count: recentRuns.length
                  })
                }}
              </span>
            </div>
            <RouterLink
              class="ai-overview__monitor-link"
              :to="monitorLink"
              data-testid="ai-overview-goto-monitor"
            >
              {{ t("aiTeam.overview.viewMonitor") }}
            </RouterLink>
          </header>

          <EmptyState
            v-if="!loading && recentRuns.length === 0"
            :title="
              errorMessage
                ? t('aiTeam.overview.loadFailed')
                : t('aiTeam.overview.runsEmpty')
            "
            data-testid="ai-overview-runs-empty"
          />

          <ol v-else class="ai-overview__runs-list">
            <li
              v-for="run in recentRuns"
              :key="run.run_id"
              class="ai-overview__run"
              :data-testid="`ai-overview-run-${run.run_id}`"
            >
              <span class="ai-overview__run-title">{{ run.task_title }}</span>
              <Badge :variant="runStatusTone(run.status)" size="sm">
                {{ run.status }}
              </Badge>
              <span class="ai-overview__run-meta">
                {{ t("aiTeam.monitor.column.attempt") }} #{{ run.attempt_no }}
              </span>
              <span class="ai-overview__run-time">
                {{ formatTime(run.created_at) }}
              </span>
            </li>
          </ol>
        </section>
      </template>
    </div>
  </AiTeamFrame>
</template>

<style scoped>
.ai-overview {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

/* 统计卡：窄屏 1 列 → 中屏 2 列 → 宽屏 4 列（照设置主列宽度自适应） */
.ai-overview__stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: var(--space-3);
}

.ai-overview__runs {
  display: flex;
  flex-direction: column;
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.ai-overview__runs-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-overview__runs-title {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.ai-overview__runs-title h3 {
  margin: 0;
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
}

.ai-overview__runs-detail {
  overflow: hidden;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-overview__monitor-link {
  flex: none;
  color: var(--ws-accent);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-decoration: none;
}

.ai-overview__monitor-link:hover {
  text-decoration: underline;
}

.ai-overview__monitor-link:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: var(--space-1);
  border-radius: var(--radius-xs);
}

.ai-overview__runs-list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-overview__run {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: var(--size-row);
  padding: var(--space-2) var(--space-4);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-overview__run:last-child {
  border-bottom: none;
}

.ai-overview__run-title {
  overflow: hidden;
  min-width: 0;
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-overview__run-meta {
  flex: none;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-overview__run-time {
  flex: none;
  min-width: 150px;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-align: right;
}
</style>
