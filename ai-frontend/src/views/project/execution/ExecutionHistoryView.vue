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
import { listTaskRuns, type TaskRun } from "@/api/execution-plans";
import { useDesignProject } from "@/views/project/design/use-design-project";
import ExecutionFrame from "./ExecutionFrame.vue";
import {
  formatDateTime,
  formatDuration,
  runLabel,
  runTone,
  verdictTone
} from "./execution-contract";

/**
 * 执行历史（S05-07，接真）。
 *
 * 数据 = GET /v1/pm/task-runs（项目级全量 Run）+ 各自的 review verdict。
 *
 * 每行分列：Run 号（kind + attempt）、任务、Agent、Run 状态、review
 * verdict、开始/时长——**Run 状态与 verdict 是两列**，不合并。
 *
 * 诚实缺位：commit 数 / 文件数在 Run 详情页逐仓给出（列表不做 N+1
 * 聚合）；分支过滤与日期区间过滤后端暂无参数，本页只做状态/kind 客户端
 * 过滤，并在页脚说明。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const runs = ref<TaskRun[]>([]);
const loading = ref(true);
const loadError = ref(false);
const statusFilter = ref("all");
const kindFilter = ref("all");

const STATUS_FILTERS = [
  "all",
  "queued",
  "claimed",
  "running",
  "succeeded",
  "failed",
  "timed_out",
  "cancelled"
] as const;

const kindLabels = computed(() => ({
  delivery: t("s05.execution.runKind.delivery"),
  review: t("s05.execution.runKind.review")
}));

async function load() {
  if (!hasProject.value) {
    runs.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    const result = await listTaskRuns({
      project_id: projectId.value,
      page: 1,
      size: 200
    });
    runs.value = result.items;
  } catch {
    loadError.value = true;
    runs.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const filtered = computed(() =>
  runs.value.filter(run => {
    if (statusFilter.value !== "all" && run.status !== statusFilter.value) {
      return false;
    }
    if (kindFilter.value !== "all" && run.run_kind !== kindFilter.value) {
      return false;
    }
    return true;
  })
);

function openRun(runId: string) {
  void router.push(`/project/execution/runs/${runId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s05.execution.module') }]"
    :title="t('s05.execution.page.historyTitle')"
    :description="t('s05.execution.page.historyDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="eh-refresh"
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

      <EmptyState
        v-else-if="!loading && runs.length === 0"
        :title="t('s05.execution.history.emptyTitle')"
        :description="t('s05.execution.history.emptyHint')"
      />

      <template v-else>
        <nav class="eh-filters" :aria-label="t('s05.execution.common.filters')">
          <button
            v-for="value in STATUS_FILTERS"
            :key="value"
            type="button"
            class="eh-filter"
            :class="{ 'is-active': statusFilter === value }"
            :aria-pressed="statusFilter === value"
            :data-testid="`eh-filter-${value}`"
            @click="statusFilter = value"
          >
            {{
              value === "all"
                ? t("s05.execution.common.all")
                : t(`s05.execution.runStatus.${value}`, value)
            }}
          </button>
          <span class="eh-sep" aria-hidden="true" />
          <button
            v-for="value in ['all', 'delivery', 'review'] as const"
            :key="`kind-${value}`"
            type="button"
            class="eh-filter"
            :class="{ 'is-active': kindFilter === value }"
            :aria-pressed="kindFilter === value"
            :data-testid="`eh-kind-${value}`"
            @click="kindFilter = value"
          >
            {{
              value === "all"
                ? t("s05.execution.common.allKinds")
                : t(`s05.execution.runKind.${value}`)
            }}
          </button>
        </nav>

        <Card class="eh-table-card">
          <table
            class="eh-table"
            :aria-label="t('s05.execution.page.historyTitle')"
          >
            <thead>
              <tr>
                <th scope="col">{{ t("s05.execution.history.colRun") }}</th>
                <th scope="col">{{ t("s05.execution.history.colTask") }}</th>
                <th scope="col">{{ t("s05.execution.history.colAgent") }}</th>
                <th scope="col">
                  {{ t("s05.execution.history.colRunStatus") }}
                </th>
                <th scope="col">{{ t("s05.execution.history.colVerdict") }}</th>
                <th scope="col">{{ t("s05.execution.history.colStarted") }}</th>
                <th scope="col">
                  {{ t("s05.execution.history.colDuration") }}
                </th>
                <th scope="col">
                  <span class="eh-sr">{{
                    t("s05.execution.common.actions")
                  }}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="run in filtered"
                :key="run.id"
                :data-testid="`eh-run-${run.id}`"
              >
                <td class="eh-mono">
                  {{ runLabel(run.run_kind, run.attempt_no, kindLabels) }}
                </td>
                <td>{{ run.task_title }}</td>
                <td class="eh-muted">
                  {{ run.agent_id ?? t("s05.execution.common.noAgent") }}
                </td>
                <td>
                  <Badge :variant="runTone(run.status)" size="sm">
                    {{ t(`s05.execution.runStatus.${run.status}`, run.status) }}
                  </Badge>
                </td>
                <td>
                  <Badge
                    v-if="run.review_verdict"
                    :variant="verdictTone(run.review_verdict)"
                    size="sm"
                  >
                    {{
                      t(
                        `s05.execution.verdict.${run.review_verdict}`,
                        run.review_verdict
                      )
                    }}
                  </Badge>
                  <span v-else class="eh-muted">
                    {{ t("s05.execution.history.noVerdict") }}
                  </span>
                </td>
                <td class="eh-muted">{{ formatDateTime(run.started_at) }}</td>
                <td class="eh-muted">
                  {{ formatDuration(run.started_at, run.finished_at) }}
                </td>
                <td>
                  <Button
                    variant="ghost"
                    size="sm"
                    :data-testid="`eh-open-${run.id}`"
                    @click="openRun(run.id)"
                  >
                    {{ t("s05.execution.common.open") }}
                  </Button>
                </td>
              </tr>
            </tbody>
          </table>
        </Card>

        <p class="eh-note">{{ t("s05.execution.history.filterNote") }}</p>
      </template>
    </ExecutionFrame>
  </PageFrame>
</template>

<style scoped>
.eh-filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1);
}

.eh-filter {
  padding: var(--space-1) var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  background: transparent;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.eh-filter:hover {
  color: var(--ws-text-primary);
}

.eh-filter:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.eh-filter.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.eh-sep {
  width: var(--space-2);
}

.eh-table-card {
  padding: 0;
  overflow: hidden;
}

.eh-table {
  width: 100%;
  border-collapse: collapse;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-text-primary);
}

.eh-table th {
  padding: var(--space-3) var(--space-4);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  text-align: left;
  color: var(--ws-text-secondary);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.eh-table td {
  padding: var(--space-3) var(--space-4);
  vertical-align: middle;
  border-bottom: var(--border-1) solid var(--ws-border);
}

.eh-table tbody tr:last-child td {
  border-bottom: none;
}

.eh-mono {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

.eh-muted {
  color: var(--ws-text-secondary);
}

.eh-note {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.eh-sr {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border-width: 0;
}
</style>
