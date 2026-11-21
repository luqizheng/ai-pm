<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import PageFrame from "@/components/shell/PageFrame.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Table from "@/components/ui/table/Table.vue";
import TableBody from "@/components/ui/table/TableBody.vue";
import TableCell from "@/components/ui/table/TableCell.vue";
import TableHead from "@/components/ui/table/TableHead.vue";
import TableHeader from "@/components/ui/table/TableHeader.vue";
import TableRow from "@/components/ui/table/TableRow.vue";
import TasksFrame from "../../TasksFrame.vue";
import TaskDetailTabs from "../TaskDetailTabs.vue";
import { listTaskRuns } from "@/api/execution-plans";
import type { TaskRun } from "@/api/execution-plans";
import { formatDateTime } from "../../tasks-contract";
import { describeError } from "@/lib/errors";

/**
 * PA-07 Runs 页（Figma 179:3603 B 页「Runs」tab）：任务的 Run 历史。
 * 数据 = GET /v1/pm/task-runs?task_id=（execution 域现成端点）；
 * Run 标签 = Delivery #attempt_no（execution-contract runLabel 口径）。
 * Harness 概念后端不存在，不做演示填充（裁决：无数据源不显示）。
 */
const { t } = useI18n();
const route = useRoute();

const taskId = computed(() => String(route.params.taskId ?? ""));

const runs = ref<TaskRun[]>([]);
const loading = ref(false);
const loadError = ref<string | null>(null);

async function load() {
  if (!taskId.value) return;
  loading.value = true;
  loadError.value = null;
  try {
    const res = await listTaskRuns({ task_id: taskId.value, size: 100 });
    runs.value = res.items ?? [];
  } catch (error) {
    loadError.value = describeError(error);
    runs.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(() => void load());
watch(taskId, value => {
  if (value) void load();
});

function runTone(status: string): string {
  if (["running", "claimed", "queued"].includes(status)) return "info";
  if (status === "succeeded") return "success";
  if (["failed", "timed_out"].includes(status)) return "danger";
  return "muted";
}

function verdictLabel(verdict: string | null): string {
  return verdict ?? "—";
}
</script>

<template>
  <PageFrame :title="t('pa07.runs.title')">
    <TasksFrame />
    <TaskDetailTabs :task-id="taskId" />

    <p v-if="loadError" class="tr-error">{{ loadError }}</p>
    <p v-else-if="loading" class="tr-note">{{ t("common.loading") }}</p>
    <EmptyState v-else-if="runs.length === 0" :title="t('pa07.runs.empty')" />

    <Table v-else data-test="task-runs-table">
      <TableHeader>
        <TableRow>
          <TableHead>{{ t("pa07.runs.colAttempt") }}</TableHead>
          <TableHead>{{ t("pa07.runs.colStatus") }}</TableHead>
          <TableHead>{{ t("pa07.runs.colVerdict") }}</TableHead>
          <TableHead>{{ t("pa07.runs.colCreatedAt") }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="run in runs" :key="run.id" data-test="task-run-row">
          <TableCell>
            <span class="tr-run-id" :data-testid="`run-attempt-${run.attempt_no}`">
              Delivery #{{ run.attempt_no }}
            </span>
          </TableCell>
          <TableCell>
            <span class="tr-status" :class="`tr-status--${runTone(run.status)}`">
              {{ run.status }}
            </span>
          </TableCell>
          <TableCell>{{ verdictLabel(run.review_verdict) }}</TableCell>
          <TableCell>
            <span class="tr-time">{{ formatDateTime(run.created_at) }}</span>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </PageFrame>
</template>

<style scoped>
.tr-error {
  margin: 0;
  color: var(--ws-color-danger-text);
}

.tr-note {
  margin: 0;
  color: var(--ws-text-secondary);
}

.tr-run-id {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-primary);
}

.tr-status {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.tr-status--info {
  color: var(--ws-color-info-text);
}

.tr-status--success {
  color: var(--ws-color-success-text);
}

.tr-status--danger {
  color: var(--ws-color-danger-text);
}

.tr-status--muted {
  color: var(--ws-text-disabled);
}

.tr-time {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}
</style>
