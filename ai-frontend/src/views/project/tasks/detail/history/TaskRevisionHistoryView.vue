<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import PageFrame from "@/components/shell/PageFrame.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import TasksFrame from "../../TasksFrame.vue";
import TaskDetailTabs from "../TaskDetailTabs.vue";
import {
  listTaskRevisions,
  type StaleActiveRun,
  type TaskRevision
} from "@/api/tasks";
import { getTaskDetail } from "@/api/task-plans";
import { formatDateTime } from "../../tasks-contract";
import { describeError } from "@/lib/errors";

/**
 * PA-07 Revision 历史页（Figma 179:3603 D 系列）。
 *
 * 时间线（最新在上：r{n} · Active / Historical · Immutable）+ 每版
 * Summary/Changed/Reason/Runs 行 + 陈旧活跃 Run 警告（「RN-87 continues
 * using r2」）+ 双选对比（并排卡片，goal/criteria/contract 三块）。
 * 数据 = GET /v1/pm/tasks/{id}/revisions（0050 读面；快照行不可变）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const taskId = computed(() => String(route.params.taskId ?? ""));

const taskTitle = ref<string>("");
const revisions = ref<TaskRevision[]>([]);
const staleRuns = ref<StaleActiveRun[]>([]);
const loading = ref(false);
const loadError = ref<string | null>(null);

const compareOld = ref<number | null>(null);
const compareNew = ref<number | null>(null);

const activeRevision = computed(
  () => revisions.value.find(rev => rev.active) ?? null
);
const compareOldRevision = computed(
  () =>
    revisions.value.find(rev => rev.revision_no === compareOld.value) ?? null
);
const compareNewRevision = computed(
  () =>
    revisions.value.find(rev => rev.revision_no === compareNew.value) ?? null
);

function criteriaCount(revision: TaskRevision | null): string {
  const criteria = revision?.acceptance_criteria as unknown as
    | Array<unknown>
    | undefined;
  return Array.isArray(criteria) ? String(criteria.length) : "—";
}

function deliverablesCount(revision: TaskRevision | null): string {
  const contract = revision?.execution_contract as
    | { deliverables?: unknown[] }
    | undefined;
  return contract && Array.isArray(contract.deliverables)
    ? String(contract.deliverables.length)
    : "—";
}

async function load() {
  if (!taskId.value) return;
  loading.value = true;
  loadError.value = null;
  try {
    const [detail, revisionsRes] = await Promise.all([
      getTaskDetail(taskId.value),
      listTaskRevisions(taskId.value)
    ]);
    taskTitle.value = detail.task.title;
    revisions.value = revisionsRes.items ?? [];
    staleRuns.value = revisionsRes.stale_active_runs ?? [];
    compareNew.value = activeRevision.value?.revision_no ?? null;
  } catch (error) {
    loadError.value = describeError(error);
  } finally {
    loading.value = false;
  }
}

onMounted(() => void load());
watch(taskId, value => {
  if (value) void load();
});

function goTask() {
  void router.push(`/project/tasks/${taskId.value}`);
}
</script>

<template>
  <PageFrame :title="t('pa07.history.title')" :description="taskTitle">
    <TasksFrame />
    <TaskDetailTabs :task-id="taskId" />

    <p v-if="loadError" class="rh-error">{{ loadError }}</p>
    <p v-else-if="loading" class="rh-note">{{ t("common.loading") }}</p>
    <EmptyState
      v-else-if="revisions.length === 0"
      :title="t('pa07.history.emptyTitle')"
      :description="t('pa07.history.emptyCopy')"
    />

    <template v-else>
      <!-- 陈旧活跃 Run 警告（Older active Run warning） -->
      <div
        v-if="staleRuns.length > 0"
        class="rh-warning"
        data-testid="rh-stale-warning"
      >
        <p class="rh-warning__title">{{ t("pa07.history.warningTitle") }}</p>
        <p v-for="run in staleRuns" :key="run.run_id" class="rh-warning__copy">
          {{
            t("pa07.history.warningCopy", {
              attempt: run.attempt_no,
              status: run.status,
              rev: run.revision_no
            })
          }}
        </p>
      </div>

      <!-- 对比选择（Compare r2 → r3） -->
      <div v-if="revisions.length > 1" class="rh-compare-bar">
        <span class="rh-compare-bar__label">{{
          t("pa07.history.compare")
        }}</span>
        <select
          v-model.number="compareOld"
          class="rh-select"
          data-test="rh-compare-old"
        >
          <option
            v-for="rev in revisions"
            :key="rev.id"
            :value="rev.revision_no"
          >
            r{{ rev.revision_no }}
          </option>
        </select>
        <span aria-hidden="true">→</span>
        <select
          v-model.number="compareNew"
          class="rh-select"
          data-test="rh-compare-new"
        >
          <option
            v-for="rev in revisions"
            :key="rev.id"
            :value="rev.revision_no"
          >
            r{{ rev.revision_no }}
          </option>
        </select>
      </div>

      <!-- 对比双卡（Revision Comparison） -->
      <div
        v-if="compareOldRevision && compareNewRevision"
        class="rh-compare"
        data-testid="rh-compare"
      >
        <div class="rh-card rh-card--old">
          <p class="rh-card__head">
            r{{ compareOldRevision.revision_no }} ·
            {{ t("pa07.history.historicalBadge") }}
          </p>
          <dl class="rh-facts">
            <div>
              <dt>{{ t("pa07.history.changedLabel") }}</dt>
              <dd>
                Criteria {{ criteriaCount(compareOldRevision) }} · Deliverables
                {{ deliverablesCount(compareOldRevision) }}
              </dd>
            </div>
            <div>
              <dt>Goal</dt>
              <dd>{{ compareOldRevision.goal ?? "—" }}</dd>
            </div>
          </dl>
        </div>
        <div class="rh-card">
          <p class="rh-card__head">
            r{{ compareNewRevision.revision_no }}
            <Badge v-if="compareNewRevision.active" variant="accent">{{
              t("pa07.history.activeBadge")
            }}</Badge>
          </p>
          <dl class="rh-facts">
            <div>
              <dt>{{ t("pa07.history.changedLabel") }}</dt>
              <dd>
                Criteria {{ criteriaCount(compareNewRevision) }} · Deliverables
                {{ deliverablesCount(compareNewRevision) }}
              </dd>
            </div>
            <div>
              <dt>Goal</dt>
              <dd>{{ compareNewRevision.goal ?? "—" }}</dd>
            </div>
          </dl>
        </div>
      </div>

      <!-- 时间线（最新在上） -->
      <ol class="rh-timeline" data-testid="rh-timeline">
        <li v-for="rev in revisions" :key="rev.id" class="rh-item">
          <div class="rh-item__head">
            <span class="rh-item__rev">r{{ rev.revision_no }}</span>
            <Badge
              :variant="rev.active ? 'accent' : 'neutral'"
              :outline="!rev.active"
              data-testid="rh-badge"
            >
              {{
                rev.active
                  ? t("pa07.history.activeBadge")
                  : t("pa07.history.historicalBadge")
              }}
            </Badge>
            <span class="rh-item__time">{{
              formatDateTime(rev.created_at)
            }}</span>
            <span
              v-if="rev.epoch_no !== rev.revision_no"
              class="rh-item__epoch"
            >
              epoch {{ rev.epoch_no }}
            </span>
          </div>
          <p class="rh-item__summary">{{ rev.goal ?? "—" }}</p>
          <dl class="rh-facts">
            <div>
              <dt>{{ t("pa07.history.reasonLabel") }}</dt>
              <dd>{{ rev.change_reason ?? "—" }}</dd>
            </div>
            <div>
              <dt>{{ t("pa07.history.runsLabel") }}</dt>
              <dd>
                {{
                  rev.run_count > 0
                    ? t("pa07.history.runsCount", { count: rev.run_count })
                    : t("pa07.history.runsNone")
                }}
              </dd>
            </div>
            <div>
              <dt>{{ t("pa07.history.changedLabel") }}</dt>
              <dd>
                Criteria {{ criteriaCount(rev) }} · Deliverables
                {{ deliverablesCount(rev) }}
              </dd>
            </div>
          </dl>
        </li>
      </ol>

      <div class="rh-foot">
        <Button
          variant="secondary"
          size="md"
          data-test="rh-back"
          @click="goTask"
        >
          {{ t("pa07.history.backToTask") }}
        </Button>
      </div>
    </template>
  </PageFrame>
</template>

<style scoped>
.rh-error {
  margin: 0;
  color: var(--ws-color-danger-text);
}

.rh-note {
  margin: 0;
  color: var(--ws-text-secondary);
}

.rh-warning {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-raised);
}

.rh-warning__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-color-warning-text);
}

.rh-warning__copy {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.rh-compare-bar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.rh-compare-bar__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.rh-select {
  height: var(--size-control-md);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  background: var(--surface-base);
  color: var(--ws-text-primary);
}

.rh-compare {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-4);
}

.rh-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-base);
}

.rh-card--old {
  background: var(--surface-raised);
}

.rh-card__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.rh-facts {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
}

.rh-facts > div {
  display: flex;
  gap: var(--space-3);
}

.rh-facts dt {
  flex-shrink: 0;
  width: 96px;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.rh-facts dd {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-primary);
}

.rh-timeline {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rh-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-base);
}

.rh-item__head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.rh-item__rev {
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.rh-item__time {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.rh-item__epoch {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-disabled);
}

.rh-item__summary {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
}

.rh-foot {
  display: flex;
  justify-content: flex-start;
}
</style>
