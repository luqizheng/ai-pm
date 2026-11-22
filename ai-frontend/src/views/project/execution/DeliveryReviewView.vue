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
  runLabel,
  runTone,
  verdictTone
} from "./execution-contract";

/**
 * 交付审查（S05-06，接真）。
 *
 * 数据 = 已成功的 delivery Run（交付候选）+ 其 review verdict。
 *
 * **完成三态分离**：本页只呈现「Agent 声称完成」（Run succeeded）与
 * 「AI-PM 检查结果」（review verdict 三值）——人工正式验收是 Task 级动作，
 * 走 /v1/pm/review-approvals 的 decision 端点，不在本页伪造。
 *
 * 诚实缺位：逐仓 commit / 测试证据的富展示在 Run 详情页（本页给入口）；
 * Agent 自撰执行报告无对应表，不展示。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const runs = ref<TaskRun[]>([]);
const loading = ref(true);
const loadError = ref(false);

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
      run_kind: "delivery",
      page: 1,
      size: 100
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

/** 可审查的交付：succeeded 的 delivery run。 */
const deliverables = computed(() =>
  runs.value.filter(run => run.status === "succeeded")
);

/** 待裁决：成功但尚未有 verdict。 */
const awaitingVerdict = computed(() =>
  deliverables.value.filter(run => !run.review_verdict)
);

function openRun(runId: string) {
  void router.push(`/project/execution/runs/${runId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s05.execution.module') }]"
    :title="t('s05.execution.page.reviewTitle')"
    :description="t('s05.execution.page.reviewDesc')"
  >
    <template #actions>
      <!-- 统一入口：正式验收裁决在审核台（人类接受 / 拒绝 / 要求修改） -->
      <Button
        variant="secondary"
        size="sm"
        data-testid="dr-review-center"
        @click="
          router.push({
            path: '/project/review-center',
            query: { kind: 'delivery', project_id: projectId }
          })
        "
      >
        {{ t("s05.execution.review.reviewCenterAction") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="dr-refresh"
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
          :title="t('s05.execution.review.triStateTitle')"
        >
          {{ t("s05.execution.review.triStateHint") }}
        </Alert>

        <Card class="dr-section" data-testid="dr-awaiting">
          <template #header>
            {{
              t("s05.execution.review.awaitingTitle", {
                count: awaitingVerdict.length
              })
            }}
          </template>
          <p v-if="awaitingVerdict.length === 0" class="dr-empty">
            {{ t("s05.execution.review.awaitingEmpty") }}
          </p>
          <ul v-else class="dr-list">
            <li
              v-for="run in awaitingVerdict"
              :key="run.id"
              class="dr-row"
              :data-testid="`dr-awaiting-${run.id}`"
            >
              <Badge :variant="runTone(run.status)" size="sm">
                {{ t(`s05.execution.runStatus.${run.status}`, run.status) }}
              </Badge>
              <span class="dr-row__title">{{ run.task_title }}</span>
              <span class="dr-meta">
                {{ runLabel(run.run_kind, run.attempt_no, kindLabels) }}
              </span>
              <span class="dr-meta">{{ formatDateTime(run.finished_at) }}</span>
              <Badge variant="warning" size="sm">
                {{ t("s05.execution.review.noVerdict") }}
              </Badge>
              <Button
                variant="ghost"
                size="sm"
                :data-testid="`dr-open-${run.id}`"
                @click="openRun(run.id)"
              >
                {{ t("s05.execution.common.open") }}
              </Button>
            </li>
          </ul>
        </Card>

        <Card class="dr-section" data-testid="dr-reviewed">
          <template #header>
            {{
              t("s05.execution.review.reviewedTitle", {
                count: deliverables.length - awaitingVerdict.length
              })
            }}
          </template>
          <p
            v-if="deliverables.length - awaitingVerdict.length === 0"
            class="dr-empty"
          >
            {{ t("s05.execution.review.reviewedEmpty") }}
          </p>
          <ul v-else class="dr-list">
            <li
              v-for="run in deliverables.filter(item => item.review_verdict)"
              :key="run.id"
              class="dr-row"
              :data-testid="`dr-reviewed-${run.id}`"
            >
              <Badge
                :variant="verdictTone(run.review_verdict)"
                size="sm"
                data-testid="dr-verdict"
              >
                {{
                  t(
                    `s05.execution.verdict.${run.review_verdict}`,
                    run.review_verdict ?? ""
                  )
                }}
              </Badge>
              <span class="dr-row__title">{{ run.task_title }}</span>
              <span class="dr-meta">
                {{ runLabel(run.run_kind, run.attempt_no, kindLabels) }}
              </span>
              <span class="dr-meta">{{ formatDateTime(run.finished_at) }}</span>
              <Button
                variant="ghost"
                size="sm"
                :data-testid="`dr-reviewed-open-${run.id}`"
                @click="openRun(run.id)"
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
.dr-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.dr-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.dr-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.dr-row__title {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

.dr-meta {
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.dr-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
