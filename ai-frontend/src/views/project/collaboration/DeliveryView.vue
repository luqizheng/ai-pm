<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { translateError } from "@/api/base";
import { listTasks } from "@/api/pm";
import {
  listCriterionVerdicts,
  listReviewFindings,
  listTaskReviewApprovals,
  type CriterionVerdict
} from "@/api/review";
import { listTaskDelivery } from "@/api/acceptance";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 F — Verification & delivery（Figma 259:593，Delivery 页签）。
 *
 * 「完成由证据赢得」：验收口径×裁决×证据表（criterion-verdicts 真实裁决；
 * evaluated=false 如实呈现未覆盖，不造 awaiting_manual 假值）+ 未完成
 * 阻塞卡（有未过口径时红卡=交付被阻塞）+ 交付产物（delivery 只读清单，
 * prepared only——验收未齐不记交付）。任务选择器同屏 B（?task_id= 深链）。
 *
 * 画布「47/48 checks」类聚合计数无独立读面——用真实口径行数统计替代。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);
const openTasks = ref<Array<{ id: string; code: string; title: string }>>([]);
const selectedTaskId = ref<string>("");
const verdicts = ref<CriterionVerdict[]>([]);
const blockerMessage = ref<string | null>(null);
const deliveryLines = ref<string[]>([]);
const lastVerifiedAt = ref<string | null>(null);
const showFailedOnly = ref(false);

const OPEN_STATES = new Set([
  "ready",
  "running",
  "reviewing",
  "verifying",
  "blocked",
  "waiting"
]);

const taskOptions = computed(() =>
  openTasks.value.map(task => ({
    value: task.id,
    label: `${task.code} · ${task.title}`
  }))
);

const visibleVerdicts = computed(() =>
  showFailedOnly.value
    ? verdicts.value.filter(
        row => row.verdict !== null && row.verdict !== "passed"
      )
    : verdicts.value
);

const passedCount = computed(
  () => verdicts.value.filter(row => row.verdict === "passed").length
);

const failedVerdicts = computed(() =>
  verdicts.value.filter(row => row.verdict && row.verdict !== "passed")
);

// 完成判定=每条口径都被 review run 覆盖且通过；「未被覆盖」≠通过
// （PA-06-05 §7.3：验收条件未满足不得标记完成——诚实优先于好看）。
const allPassed = computed(
  () =>
    verdicts.value.length > 0 &&
    verdicts.value.every(row => rowEvaluated(row) && row.verdict === "passed")
);

function verdictLabel(verdict: CriterionVerdict): string {
  if (!rowEvaluated(verdict)) {
    return t("pa06.delivery.notCovered");
  }
  return verdict.verdict === "passed"
    ? t("pa06.delivery.passed")
    : t("pa06.delivery.failed", { count: 1 });
}

function rowEvaluated(row: CriterionVerdict): boolean {
  return row.evaluated && row.verdict !== null;
}

function evidenceRef(row: CriterionVerdict): string {
  const evidence = row.evidence as { ref?: string; path?: string } | null;
  return evidence?.ref ?? evidence?.path ?? "—";
}

async function load(): Promise<void> {
  if (!hasProject.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const id = projectId.value;
    const fromQuery = route.query.task_id;
    const rows = await listTasks({ project_id: id, page: 1, size: 200 });
    openTasks.value = (rows.items ?? [])
      .filter(
        task => OPEN_STATES.has(task.status) || task.status === "completed"
      )
      .map(task => ({ id: task.id, code: task.code, title: task.title }));
    const wanted =
      typeof fromQuery === "string" && fromQuery
        ? fromQuery
        : (openTasks.value[0]?.id ?? "");
    selectedTaskId.value = wanted;
    if (!wanted) {
      verdicts.value = [];
      deliveryLines.value = [];
      blockerMessage.value = null;
      loading.value = false;
      return;
    }
    const [verdictRows, findings, delivery, approvals] = await Promise.all([
      listCriterionVerdicts(wanted),
      listReviewFindings(wanted).catch(() => ({ items: [] })),
      listTaskDelivery(wanted).catch(() => null),
      listTaskReviewApprovals(wanted).catch(() => ({ items: [] }))
    ]);
    verdicts.value = (verdictRows.items ?? [])
      .slice()
      .sort((a, b) => a.ordinal - b.ordinal);
    const blockers = (findings.items ?? []).filter(
      finding => finding.severity === "blocker"
    );
    blockerMessage.value =
      blockers[0]?.message ?? (failedVerdicts.value.length > 0 ? null : null);
    deliveryLines.value = (delivery?.items ?? []).map(row =>
      [row.branch ?? row.repository_id, row.delivery_state]
        .filter(Boolean)
        .join(" · ")
    );
    lastVerifiedAt.value = approvals.items?.[0]?.decided_at ?? null;
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

function pickTask(value: string): void {
  void router.replace({
    path: "/project/collaboration/delivery",
    query: { project_id: projectId.value, task_id: value }
  });
}

function toggleFailedFilter(): void {
  showFailedOnly.value = !showFailedOnly.value;
}

onMounted(load);
watch([projectId, () => route.query.task_id], load);
</script>

<template>
  <CollaborationFrame>
    <ScreenFeedback
      v-if="loading"
      state="loading"
      :title="$t('pa06.delivery.loadingEyebrow')"
    />
    <ScreenFeedback
      v-else-if="error"
      state="error"
      :description="error"
      @retry="load"
    />
    <template v-else>
      <template v-if="!hasProject">
        <ScreenFeedback state="empty" :title="$t('pa06.frame.noProject')" />
      </template>
      <ScreenFeedback
        v-else-if="openTasks.length === 0"
        state="empty"
        :title="$t('pa06.delivery.emptyTitle')"
        :description="$t('pa06.delivery.emptyHint')"
      />
      <CollabLayout v-else>
        <div class="dl-head">
          <div class="dl-head__eyebrow-row">
            <span class="dl-eyebrow">
              {{
                $t("pa06.delivery.eyebrow", {
                  code:
                    openTasks.find(task => task.id === selectedTaskId)?.code ??
                    ""
                })
              }}
            </span>
            <Badge :variant="allPassed ? 'success' : 'accent'">
              {{
                allPassed
                  ? $t("pa06.delivery.completeBadge")
                  : $t("pa06.delivery.incompleteBadge", {
                      passed: passedCount,
                      total: verdicts.length
                    })
              }}
            </Badge>
          </div>
          <!-- 三态分离导航：本页=证据呈现（只读）；正式验收裁决在审核台 -->
          <div class="dl-head__nav">
            <RouterLink
              class="dl-review-link"
              :to="{
                path: '/project/review-center',
                query: { kind: 'delivery', project_id: projectId }
              }"
              data-testid="dl-review-center-link"
            >
              {{ $t("pa06.delivery.reviewCenterLink") }}
            </RouterLink>
            <RouterLink
              v-if="selectedTaskId"
              class="dl-review-link"
              :to="{
                path: '/project/execution/review',
                query: { project_id: projectId }
              }"
              data-testid="dl-execution-review-link"
            >
              {{ $t("pa06.delivery.executionReviewLink") }}
            </RouterLink>
          </div>
          <h2 class="dl-head__title">{{ $t("pa06.delivery.headline") }}</h2>
          <p class="dl-head__meta">
            {{ $t("pa06.delivery.headlineMeta") }}
          </p>
          <Button
            variant="secondary"
            size="sm"
            data-testid="dl-failed-filter"
            @click="toggleFailedFilter"
          >
            {{
              showFailedOnly
                ? $t("pa06.delivery.showAll")
                : $t("pa06.delivery.inspectFailed")
            }}
          </Button>
          <Select
            :model-value="selectedTaskId"
            :options="taskOptions"
            :aria-label="$t('pa06.execution.pickTask')"
            data-testid="dl-task-picker"
            @update:model-value="pickTask"
          />
        </div>

        <!-- 验收口径×裁决×证据表 -->
        <div class="dl-card" data-testid="dl-evidence">
          <div class="dl-table__head">
            <span>{{ $t("pa06.delivery.colCriteria") }}</span>
            <span>{{ $t("pa06.delivery.colResult") }}</span>
            <span>{{ $t("pa06.delivery.colEvidence") }}</span>
          </div>
          <div
            v-for="row in visibleVerdicts"
            :key="row.criterion_id"
            class="dl-table__row"
            :data-testid="`dl-verdict-${row.ordinal}`"
          >
            <span class="dl-table__text">{{ row.text }}</span>
            <span
              class="dl-table__verdict"
              :class="{
                'is-pass': row.verdict === 'passed',
                'is-fail': rowEvaluated(row) && row.verdict !== 'passed'
              }"
            >
              {{ verdictLabel(row) }}
            </span>
            <span class="dl-table__evidence">{{ evidenceRef(row) }}</span>
          </div>
          <p v-if="visibleVerdicts.length === 0" class="dl-table__empty">
            {{ $t("pa06.delivery.noVerdicts") }}
          </p>
        </div>

        <!-- 交付阻塞卡（未全过即阻塞：未通过与未覆盖都算未完成） -->
        <div v-if="!allPassed" class="dl-blocked" data-testid="dl-blocked">
          <p class="dl-blocked__title">
            {{
              $t("pa06.delivery.blockedTitle", {
                passed: passedCount,
                total: verdicts.length
              })
            }}
          </p>
          <p class="dl-blocked__meta">
            {{ blockerMessage || $t("pa06.delivery.blockedDefault") }}
          </p>
        </div>

        <!-- 交付产物 -->
        <span class="dl-eyebrow">{{
          $t("pa06.delivery.artifactsEyebrow")
        }}</span>
        <div class="dl-artifacts" data-testid="dl-artifacts">
          <p
            v-for="line in deliveryLines"
            :key="line"
            class="dl-artifacts__line"
          >
            {{ line }}
          </p>
          <p v-if="deliveryLines.length === 0" class="dl-artifacts__line">
            {{ $t("pa06.delivery.noArtifacts") }}
          </p>
          <p class="dl-artifacts__note">
            {{
              allPassed
                ? $t("pa06.delivery.artifactsReady")
                : $t("pa06.delivery.artifactsPreparedOnly")
            }}
          </p>
        </div>

        <template #context>
          <CollabContextPanel :title="$t('pa06.delivery.ctxTitle')">
            <p class="dl-ctx-text">{{ $t("pa06.delivery.ctxPolicy") }}</p>
            <CollabContextSection :label="$t('pa06.delivery.ctxResults')">
              {{
                $t("pa06.delivery.ctxResultsText", {
                  passed: passedCount,
                  total: verdicts.length
                })
              }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.delivery.ctxOutstanding')">
              {{
                failedVerdicts.length > 0
                  ? $t("pa06.delivery.ctxOutstandingText", {
                      count: failedVerdicts.length
                    })
                  : $t("pa06.delivery.ctxNoOutstanding")
              }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.delivery.ctxHistory')">
              {{ $t("pa06.delivery.ctxHistoryText") }}
            </CollabContextSection>
            <p class="dl-ctx-note">{{ $t("pa06.delivery.ctxNote") }}</p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.dl-eyebrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.dl-head {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
}

.dl-head__nav {
  display: flex;
  gap: var(--space-3);
}

.dl-review-link {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-accent);
  text-decoration: none;
}

.dl-review-link:hover {
  text-decoration: underline;
}

.dl-head__eyebrow-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: var(--space-3);
}

.dl-head__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: 1.375rem;
  font-weight: var(--weight-semibold);
  letter-spacing: -0.018em;
  color: var(--text-primary);
}

.dl-head__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.dl-card {
  display: flex;
  flex-direction: column;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-3) var(--space-4);
}

.dl-table__head,
.dl-table__row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 8rem 10rem;
  gap: var(--space-3);
  align-items: center;
}

.dl-table__head {
  padding: var(--space-2) 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
  border-bottom: var(--border-1) solid var(--border-subtle);
}

.dl-table__row {
  padding: var(--space-3) 0;
  border-bottom: var(--border-1) solid var(--border-subtle);
}

.dl-table__row:last-child {
  border-bottom: none;
}

.dl-table__text {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.dl-table__verdict.is-pass {
  color: var(--ws-color-success-text, var(--text-primary));
}

.dl-table__verdict.is-fail {
  color: var(--ws-color-danger-text);
}

.dl-table__evidence {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-accent);
}

.dl-table__empty {
  margin: 0;
  padding: var(--space-3) 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.dl-blocked {
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--ws-color-danger-text) 8%, transparent);
  padding: var(--space-3) var(--space-4);
}

.dl-blocked__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--ws-color-danger-text);
}

.dl-blocked__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.dl-artifacts {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.dl-artifacts__line {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-accent);
}

.dl-artifacts__note {
  margin: var(--space-1) 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.dl-ctx-text {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.dl-ctx-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: 1.45;
  color: var(--text-secondary);
}
</style>
