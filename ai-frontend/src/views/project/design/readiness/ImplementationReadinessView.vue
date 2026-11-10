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
  evaluateReadiness,
  listBaselineTasks,
  listImplementationBaselines,
  type BaselineTaskRow,
  type GateReport,
  type ImplementationBaseline
} from "@/api/designs";
import DesignFrame from "../DesignFrame.vue";
import { formatDateTime } from "../design-contract";
import { useDesignProject } from "../use-design-project";

/**
 * S03-06 实施准备页（docs/ui/s03 页面结构 §8「Implementation Readiness」）：
 * 准入检查 / 基线 / 交接 S04。
 *
 * 三个层级必须视觉可区分（§6.3）：
 *   1. **准入检查** = Readiness Gate 五条件 dry-run（GET，纯读，不改状态）；
 *   2. **基线** = 已冻结的实施依据快照（冻结后不可变，只能被显式替代）；
 *   3. **交接 S04** = 以该基线为实施依据的任务（task.implementation_baseline_id）。
 *
 * 「冻结基线」是确认类命令（真人门 + 门禁全过），本页不提供冻结入口——
 * 写路径走命令端点，UI 只读呈现事实与门禁判定。
 */
const { t } = useI18n();
const { projectId, hasProject } = useDesignProject();

const report = ref<GateReport | null>(null);
const baselines = ref<ImplementationBaseline[]>([]);
const selectedBaselineId = ref("");
const baselineTasks = ref<BaselineTaskRow[]>([]);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!hasProject.value) {
    report.value = null;
    baselines.value = [];
    baselineTasks.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    const [gate, list] = await Promise.all([
      evaluateReadiness(projectId.value),
      listImplementationBaselines(projectId.value)
    ]);
    report.value = gate;
    baselines.value = list;
    selectedBaselineId.value = list[0]?.id ?? "";
    await loadBaselineTasks(selectedBaselineId.value);
  } catch {
    loadError.value = true;
    report.value = null;
    baselines.value = [];
    baselineTasks.value = [];
  } finally {
    loading.value = false;
  }
}

async function loadBaselineTasks(baselineId: string) {
  if (!baselineId) {
    baselineTasks.value = [];
    return;
  }
  try {
    baselineTasks.value = await listBaselineTasks(baselineId);
  } catch {
    baselineTasks.value = [];
  }
}

onMounted(load);
watch(projectId, load);

function selectBaseline(baselineId: string) {
  selectedBaselineId.value = baselineId;
  void loadBaselineTasks(baselineId);
}

const selectedBaseline = computed(
  () =>
    baselines.value.find(item => item.id === selectedBaselineId.value) ?? null
);

/** 门禁五条件的 i18n 键（condition 值为库内固定词表）。 */
function conditionLabel(condition: string): string {
  const key = `s03.design.gate.${condition}`;
  const translated = t(key);
  return translated === key ? condition : translated;
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s03.design.module') }]"
    :title="t('s03.design.page.readinessTitle')"
    :description="t('s03.design.page.readinessDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="readiness-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s03.design.common.refresh") }}
      </Button>
    </template>

    <DesignFrame>
      <Alert
        v-if="loadError"
        variant="danger"
        size="sm"
        :title="t('s03.design.common.errorTitle')"
      />

      <EmptyState
        v-if="!hasProject"
        :title="t('s03.design.common.noProjectTitle')"
        :description="t('s03.design.common.noProjectHint')"
      />

      <template v-else>
        <Card class="gate-card" data-testid="readiness-gate">
          <template #header>
            <div class="gate-card__head">
              <span>{{ t("s03.design.readiness.gateTitle") }}</span>
              <Badge
                v-if="report"
                :variant="report.ready ? 'success' : 'warning'"
                size="sm"
                data-testid="readiness-gate-verdict"
              >
                {{
                  report.ready
                    ? t("s03.design.readiness.ready")
                    : t("s03.design.readiness.notReady")
                }}
              </Badge>
            </div>
          </template>

          <p
            v-if="!report || report.conditions.length === 0"
            class="gate-empty"
          >
            {{ t("s03.design.readiness.noApprovedRevisions") }}
          </p>
          <ul v-else class="gate-list">
            <li
              v-for="condition in report.conditions"
              :key="condition.condition"
              class="gate-row"
              :class="{ 'is-failed': !condition.passed }"
              :data-testid="`gate-condition-${condition.condition}`"
            >
              <Badge
                :variant="condition.passed ? 'success' : 'danger'"
                size="sm"
              >
                {{
                  condition.passed
                    ? t("s03.design.readiness.passed")
                    : t("s03.design.readiness.failed")
                }}
              </Badge>
              <span class="gate-row__name">{{
                conditionLabel(condition.condition)
              }}</span>
              <ul v-if="condition.details.length > 0" class="gate-details">
                <li
                  v-for="detail in condition.details"
                  :key="detail"
                  class="gate-detail"
                >
                  {{ detail }}
                </li>
              </ul>
            </li>
          </ul>
        </Card>

        <Card class="baselines-card">
          <template #header>{{
            t("s03.design.readiness.baselinesTitle")
          }}</template>
          <p v-if="baselines.length === 0" class="gate-empty">
            {{ t("s03.design.readiness.noBaselines") }}
          </p>
          <ul v-else class="baseline-list">
            <li
              v-for="baseline in baselines"
              :key="baseline.id"
              class="baseline-row"
            >
              <button
                type="button"
                class="baseline-row__button"
                :class="{ 'is-active': baseline.id === selectedBaselineId }"
                :aria-pressed="baseline.id === selectedBaselineId"
                :data-testid="`baseline-${baseline.code}`"
                @click="selectBaseline(baseline.id)"
              >
                <span class="baseline-row__code">{{ baseline.code }}</span>
                <Badge
                  :variant="
                    baseline.status === 'frozen' ? 'success' : 'neutral'
                  "
                  size="sm"
                >
                  {{
                    baseline.status === "frozen"
                      ? t("s03.design.readiness.frozen")
                      : t("s03.design.readiness.superseded")
                  }}
                </Badge>
                <span class="baseline-row__meta">
                  {{ formatDateTime(baseline.frozen_at) }}
                </span>
              </button>
            </li>
          </ul>
        </Card>

        <Card
          v-if="selectedBaseline"
          class="handoff-card"
          data-testid="readiness-handoff"
        >
          <template #header>
            {{ t("s03.design.readiness.handoffTitle") }}
          </template>
          <p class="handoff-desc">
            {{
              t("s03.design.readiness.handoffHint", {
                code: selectedBaseline.code
              })
            }}
          </p>
          <p v-if="baselineTasks.length === 0" class="gate-empty">
            {{ t("s03.design.readiness.noHandoffTasks") }}
          </p>
          <ul v-else class="handoff-list">
            <li
              v-for="task in baselineTasks"
              :key="task.id"
              class="handoff-row"
              :data-testid="`handoff-task-${task.id}`"
            >
              <span class="handoff-row__title">{{ task.title }}</span>
              <Badge variant="neutral" size="sm">
                {{ t(`s04.tasks.status.${task.status}`) }}
              </Badge>
              <span class="handoff-row__meta">
                {{ t(`s04.tasks.priority.${task.priority}`) }}
              </span>
            </li>
          </ul>
        </Card>
      </template>
    </DesignFrame>
  </PageFrame>
</template>

<style scoped>
.gate-card,
.baselines-card,
.handoff-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.gate-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.gate-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.gate-list,
.baseline-list,
.handoff-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.gate-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.gate-row.is-failed {
  color: var(--ws-color-danger-text);
}

.gate-row__name {
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.gate-details {
  flex-basis: 100%;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.gate-detail {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.baseline-row__button {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  text-align: left;
  background: transparent;
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.baseline-row__button:hover {
  background: var(--surface-base);
}

.baseline-row__button:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.baseline-row__button.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.baseline-row__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  font-weight: var(--weight-semibold);
}

.baseline-row__meta {
  color: var(--ws-text-secondary);
}

.handoff-desc {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.handoff-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.handoff-row__title {
  color: var(--ws-text-primary);
}

.handoff-row__meta {
  color: var(--ws-text-secondary);
}
</style>
