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
  type HumanAttentionQueue
} from "@/api/execution-plans";
import { useDesignProject } from "@/views/project/design/use-design-project";
import ExecutionFrame from "./ExecutionFrame.vue";
import { formatDateTime, waitingTone } from "./execution-contract";

/**
 * 人工介入（S05-04，接真）。
 *
 * 五场景按 **waiting reason 五值**分组呈现——每值是不同的处置路径
 * （澄清 / 决策 / 审批 / 外部依赖 / 无可用执行者），不做「等待中」合并。
 * 待答交互带出问题原文（人工答复的对象）。
 *
 * 诚实缺位：权限申请（permission request）无实体表；答复动作走
 * POST /interactions/{id}/responses（本页只读呈现，写入口在详情页）。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const queue = ref<HumanAttentionQueue | null>(null);
const loading = ref(true);
const loadError = ref(false);

const REASONS = [
  "needs_info",
  "needs_decision",
  "needs_approval",
  "external_dependency",
  "no_executor_available"
] as const;

async function load() {
  if (!hasProject.value) {
    queue.value = null;
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    queue.value = await getHumanAttentionQueue(projectId.value);
  } catch {
    loadError.value = true;
    queue.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const interactions = computed(() => queue.value?.interactions ?? []);

function waitingBy(reason: string) {
  return (queue.value?.waiting ?? []).filter(item => item.reason === reason);
}

function openRun(runId: string) {
  void router.push(`/project/execution/runs/${runId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s05.execution.module') }]"
    :title="t('s05.execution.page.interventionTitle')"
    :description="t('s05.execution.page.interventionDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="hi-refresh"
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
        <Card
          v-for="reason in REASONS"
          :key="reason"
          class="hi-section"
          :data-testid="`hi-section-${reason}`"
        >
          <template #header>
            <div class="hi-head">
              <Badge :variant="waitingTone(reason)" size="sm">
                {{ t(`s05.execution.waitingReason.${reason}`) }}
              </Badge>
              <span class="hi-count">{{ waitingBy(reason).length }}</span>
            </div>
          </template>
          <p v-if="waitingBy(reason).length === 0" class="hi-empty">
            {{ t("s05.execution.intervention.sectionEmpty") }}
          </p>
          <ul v-else class="hi-list">
            <li
              v-for="item in waitingBy(reason)"
              :key="item.id"
              class="hi-row"
              :data-testid="`hi-waiting-${item.id}`"
            >
              <span class="hi-row__title">{{ item.task_title }}</span>
              <Badge v-if="item.task_status" variant="neutral" size="sm">
                {{
                  t(
                    `s05.execution.taskStatus.${item.task_status}`,
                    item.task_status
                  )
                }}
              </Badge>
              <span class="hi-row__meta">
                {{ t("s05.execution.intervention.since") }}
                {{ formatDateTime(item.since) }}
              </span>
              <span v-if="item.deadline" class="hi-row__meta">
                {{ t("s05.execution.intervention.deadline") }}
                {{ formatDateTime(item.deadline) }}
              </span>
            </li>
          </ul>
        </Card>

        <Card class="hi-section" data-testid="hi-interactions">
          <template #header>
            {{
              t("s05.execution.intervention.interactionsTitle", {
                count: interactions.length
              })
            }}
          </template>
          <p v-if="interactions.length === 0" class="hi-empty">
            {{ t("s05.execution.intervention.interactionsEmpty") }}
          </p>
          <ul v-else class="hi-list">
            <li
              v-for="item in interactions"
              :key="item.id"
              class="hi-row"
              :data-testid="`hi-interaction-${item.id}`"
            >
              <span class="hi-row__title">{{ item.title }}</span>
              <Badge variant="warning" size="sm">
                {{
                  t(`s05.execution.waitingReason.${item.reason}`, item.reason)
                }}
              </Badge>
              <span class="hi-row__question">{{ item.question }}</span>
              <span class="hi-row__meta">
                {{ t("s05.execution.intervention.deadline") }}
                {{ formatDateTime(item.deadline_at) }}
              </span>
              <Button
                variant="ghost"
                size="sm"
                :data-testid="`hi-open-${item.id}`"
                @click="openRun(item.run_id)"
              >
                {{ t("s05.execution.common.open") }}
              </Button>
            </li>
          </ul>
        </Card>
      </template>
    </ExecutionFrame>
  </PageFrame>
</template>

<style scoped>
.hi-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.hi-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.hi-count {
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-secondary);
}

.hi-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.hi-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.hi-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.hi-row__title {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

.hi-row__question {
  flex-basis: 100%;
  color: var(--ws-text-primary);
}

.hi-row__meta {
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}
</style>
