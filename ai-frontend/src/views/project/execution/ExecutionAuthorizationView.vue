<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import {
  getHumanAttentionQueue,
  listTaskRuns,
  type HumanAttentionQueue,
  type TaskRun
} from "@/api/execution-plans";
import { useDesignProject } from "@/views/project/design/use-design-project";
import ExecutionFrame from "./ExecutionFrame.vue";
import { formatDateTime, runLabel, waitingTone } from "./execution-contract";

/**
 * 执行授权（S05-05，接真）。
 *
 * 授权队列 = 处于 `awaiting_decision` 且 reason ∈ {needs_decision,
 * needs_approval} 的等待项 + ready 态任务。
 *
 * **本页只读**：授予/拒绝是一次性授权命令（真人门），本批不落地写入口；
 * 页面呈现「谁在等授权、等什么、等了多久」，不做伪授权按钮。
 *
 * **任务级一次性授权 ≠ ACP-07 Run 级 Deadline 重授权**：后者在 Run 详情
 * 页的授权台账里，二者不可混用。
 */
const { t } = useI18n();
const { projectId, hasProject } = useDesignProject();

const queue = ref<HumanAttentionQueue | null>(null);
const runs = ref<TaskRun[]>([]);
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
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    const [attention, runList] = await Promise.all([
      getHumanAttentionQueue(projectId.value),
      listTaskRuns({ project_id: projectId.value, page: 1, size: 100 })
    ]);
    queue.value = attention;
    runs.value = runList.items;
  } catch {
    loadError.value = true;
    queue.value = null;
    runs.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

/** 需要授权的等待项（决策 / 审批两类）。 */
const pending = computed(() =>
  (queue.value?.waiting ?? []).filter(
    item => item.reason === "needs_decision" || item.reason === "needs_approval"
  )
);

/** 排队中、尚未领取的 Run（执行队列）。 */
const queued = computed(() =>
  runs.value.filter(run => run.status === "queued")
);
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s05.execution.module') }]"
    :title="t('s05.execution.page.authorizationTitle')"
    :description="t('s05.execution.page.authorizationDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="ea-refresh"
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
        <Alert
          variant="info"
          size="sm"
          :title="t('s05.execution.authorization.readOnlyTitle')"
        >
          {{ t("s05.execution.authorization.readOnlyHint") }}
        </Alert>

        <Card class="ea-section" data-testid="ea-pending">
          <template #header>
            {{
              t("s05.execution.authorization.pendingTitle", {
                count: pending.length
              })
            }}
          </template>
          <p v-if="pending.length === 0" class="ea-empty">
            {{ t("s05.execution.authorization.pendingEmpty") }}
          </p>
          <ul v-else class="ea-list">
            <li
              v-for="item in pending"
              :key="item.id"
              class="ea-row"
              :data-testid="`ea-pending-${item.reason}`"
            >
              <Badge :variant="waitingTone(item.reason)" size="sm">
                {{
                  t(`s05.execution.waitingReason.${item.reason}`, item.reason)
                }}
              </Badge>
              <span class="ea-row__title">{{ item.task_title }}</span>
              <span class="ea-meta">
                {{ t("s05.execution.authorization.since") }}
                {{ formatDateTime(item.since) }}
              </span>
              <span v-if="item.deadline" class="ea-meta">
                {{ t("s05.execution.authorization.deadline") }}
                {{ formatDateTime(item.deadline) }}
              </span>
            </li>
          </ul>
        </Card>

        <Card class="ea-section" data-testid="ea-queue">
          <template #header>
            {{
              t("s05.execution.authorization.queueTitle", {
                count: queued.length
              })
            }}
          </template>
          <p v-if="queued.length === 0" class="ea-empty">
            {{ t("s05.execution.authorization.queueEmpty") }}
          </p>
          <ul v-else class="ea-list">
            <li
              v-for="run in queued"
              :key="run.id"
              class="ea-row"
              :data-testid="`ea-queued-${run.id}`"
            >
              <span class="ea-row__title">{{ run.task_title }}</span>
              <span class="ea-meta">
                {{ runLabel(run.run_kind, run.attempt_no, kindLabels) }}
              </span>
              <span class="ea-meta">{{ formatDateTime(run.created_at) }}</span>
            </li>
          </ul>
        </Card>
      </template>
    </ExecutionFrame>
  </PageFrame>
</template>

<style scoped>
.ea-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ea-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ea-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.ea-row__title {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

.ea-meta {
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.ea-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
