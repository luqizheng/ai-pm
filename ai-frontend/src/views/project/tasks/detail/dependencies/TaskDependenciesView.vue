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
  addTaskDependency,
  listTaskDependencies,
  removeTaskDependency,
  type TaskDependencyEdge
} from "@/api/tasks";
import { listTasks } from "@/api/pm";
import { getTaskDetail, type TaskDetailView } from "@/api/task-plans";
import {
  aggregateStatusLabel,
  aggregateStatusTone,
  priorityGrade
} from "../../tasks-contract";

/**
 * PA-07 任务依赖页（Figma 179:3603 B 系列）。
 *
 * Blocked Reason Panel（或全满足面板）+ Depends On / Blocks 边行（每边
 * required_condition + satisfied 求值）+ 添加依赖三步流（选前置→设条件→复核，
 * 照后端 add_dependency_with 契约）+ 移除确认 + 空态/只读（终态任务）变体。
 * 环路由后端 409 拒绝（DEPENDENCY_CYCLE），本页原样呈现错误。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const taskId = computed(() => route.params.taskId as string);

const detail = ref<TaskDetailView | null>(null);
const edges = ref<{
  upstream: TaskDependencyEdge[];
  downstream: TaskDependencyEdge[];
}>({
  upstream: [],
  downstream: []
});
const loading = ref(false);
const loadError = ref<string | null>(null);

const candidateTasks = ref<Array<{ id: string; title: string; code: string }>>(
  []
);

const isClosed = computed(() =>
  ["completed", "failed", "cancelled", "superseded"].includes(
    detail.value?.task.status ?? ""
  )
);

const totalUpstream = computed(() => edges.value.upstream.length);
const metUpstream = computed(
  () => edges.value.upstream.filter(edge => edge.satisfied).length
);

function conditionLabel(edge: TaskDependencyEdge): string {
  switch (edge.required_condition) {
    case "accepted":
      return t("pa07.dependencies.conditionAccepted");
    case "deliverable_approved":
      return t("pa07.dependencies.conditionDeliverable");
    default:
      return t("pa07.dependencies.conditionCompleted");
  }
}

function deliverableLabel(edge: TaskDependencyEdge): string {
  if (
    edge.required_condition !== "deliverable_approved" ||
    !edge.required_deliverable_ref
  ) {
    return t("pa07.dependencies.noDeliverable");
  }
  const ref = edge.required_deliverable_ref as { id?: string; path?: string };
  return String(ref.id ?? ref.path ?? t("pa07.dependencies.noDeliverable"));
}

// ───────────────────────────── 添加依赖三步流（B2/B3/B9）

const wizardOpen = ref(false);
const wizardStep = ref<1 | 2 | 3>(1);
const wizardPredecessor = ref("");
const wizardCondition = ref<"completed" | "accepted" | "deliverable_approved">(
  "completed"
);
const wizardDeliverableRef = ref("");
const wizardError = ref<string | null>(null);
const saving = ref(false);

// 移除确认（C2）
const removeTarget = ref<TaskDependencyEdge | null>(null);
const removeError = ref<string | null>(null);

const wizardPickable = computed(() =>
  candidateTasks.value.filter(task => task.id !== taskId.value)
);

const wizardPickLabel = computed(() => {
  const picked = wizardPickable.value.find(
    task => task.id === wizardPredecessor.value
  );
  return picked ? `${picked.code}  ${picked.title}` : "";
});

function openWizard() {
  wizardOpen.value = true;
  wizardStep.value = 1;
  wizardPredecessor.value = "";
  wizardCondition.value = "completed";
  wizardDeliverableRef.value = "";
  wizardError.value = null;
}

function closeWizard() {
  wizardOpen.value = false;
}

async function submitWizard() {
  if (!wizardPredecessor.value) return;
  saving.value = true;
  wizardError.value = null;
  try {
    await addTaskDependency(taskId.value, wizardPredecessor.value, {
      required_condition: wizardCondition.value,
      ...(wizardCondition.value === "deliverable_approved" &&
      wizardDeliverableRef.value.trim()
        ? {
            required_deliverable_ref: { id: wizardDeliverableRef.value.trim() }
          }
        : {})
    });
    wizardOpen.value = false;
    await load();
  } catch (error) {
    wizardError.value = describeError(error);
  } finally {
    saving.value = false;
  }
}

async function confirmRemove() {
  if (!removeTarget.value) return;
  removeError.value = null;
  try {
    await removeTaskDependency(taskId.value, removeTarget.value.id);
    removeTarget.value = null;
    await load();
  } catch (error) {
    removeError.value = describeError(error);
  }
}

async function load() {
  loading.value = true;
  loadError.value = null;
  try {
    const [detailRes, depsRes, tasksRes] = await Promise.all([
      getTaskDetail(taskId.value),
      listTaskDependencies(taskId.value),
      listTasks({ size: 200 })
    ]);
    detail.value = detailRes;
    edges.value = {
      upstream: depsRes.upstream ?? [],
      downstream: depsRes.downstream ?? []
    };
    candidateTasks.value = (tasksRes.items ?? [])
      .filter(task => task.id !== taskId.value)
      .map(task => ({ id: task.id, title: task.title, code: task.code }));
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

function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

function openTask(id: string) {
  void router.push(`/project/tasks/${id}`);
}
</script>

<template>
  <PageFrame
    :title="t('pa07.dependencies.title')"
    :description="detail?.task.title ?? ''"
  >
    <TasksFrame />
    <TaskDetailTabs :task-id="taskId" />

    <p v-if="loadError" class="td-error">{{ loadError }}</p>
    <p v-else-if="loading" class="td-note">{{ t("common.loading") }}</p>
    <EmptyState
      v-else-if="!detail"
      :title="t('pa07.dependencies.loadFailed')"
    />

    <template v-else>
      <div class="td-objective" data-testid="dep-objective">
        <span class="td-objective__code" data-testid="dep-task-code">{{
          detail.task.code
        }}</span>
        <span class="td-objective__title">{{ detail.task.title }}</span>
        <span
          class="td-objective__status"
          :class="`td-objective__status--${aggregateStatusTone(detail.task.status)}`"
        >
          {{ aggregateStatusLabel(detail.task.status) }} ·
          {{ priorityGrade(detail.task.priority) }}
        </span>
      </div>

      <!-- Blocked / 全满足面板（PA-07 / Blocked Reason Panel 与 Eligibility Summary） -->
      <div
        class="td-gate"
        :class="{ 'td-gate--satisfied': totalUpstream === metUpstream }"
        data-testid="dep-gate"
      >
        <template v-if="totalUpstream === 0">
          <p class="td-gate__title">{{ t("pa07.dependencies.emptyTitle") }}</p>
          <p class="td-gate__copy">{{ t("pa07.dependencies.emptyCopy") }}</p>
        </template>
        <template v-else-if="metUpstream < totalUpstream">
          <p class="td-gate__title">
            {{ t("pa07.dependencies.blockedTitle") }}
          </p>
          <p class="td-gate__copy">
            {{ t("pa07.dependencies.blockedCopy") }}
          </p>
        </template>
        <template v-else>
          <p class="td-gate__title">
            {{ t("pa07.dependencies.allSatisfiedTitle") }}
          </p>
          <p class="td-gate__copy">
            {{ t("pa07.dependencies.allSatisfiedCopy") }}
          </p>
        </template>
        <p
          v-if="totalUpstream > 0"
          class="td-gate__summary"
          data-testid="dep-conditions-summary"
        >
          {{
            t("pa07.dependencies.conditionsSummary", {
              met: metUpstream,
              total: totalUpstream
            })
          }}
        </p>
      </div>

      <div class="td-actions">
        <Button
          v-if="!isClosed"
          variant="primary"
          size="md"
          data-test="dep-add-dependency"
          @click="openWizard"
        >
          {{ t("pa07.dependencies.addDependency") }}
        </Button>
        <Badge v-else variant="neutral" :outline="true">{{
          t("pa07.dependencies.lockedTitle")
        }}</Badge>
      </div>

      <section class="td-section">
        <p class="td-section__caption">
          {{ t("pa07.dependencies.dependsOn") }}
        </p>
        <div
          v-if="edges.upstream.length === 0"
          class="td-row td-row--muted"
          data-test="dep-upstream-empty"
        >
          {{ t("pa07.relationships.emptyDependsOn") }}
        </div>
        <div
          v-for="edge in edges.upstream"
          :key="edge.id"
          class="td-row"
          data-test="dep-upstream-row"
        >
          <div class="td-row__main">
            <button class="td-row__title" @click="openTask(edge.id)">
              {{ edge.title }}
            </button>
            <span class="td-row__meta">
              {{ aggregateStatusLabel(edge.status) }} ·
              {{ conditionLabel(edge) }} ·
              {{ deliverableLabel(edge) }}
            </span>
          </div>
          <span
            class="td-row__result"
            :class="{ 'is-satisfied': edge.satisfied }"
            data-testid="dep-edge-satisfied"
          >
            {{
              edge.satisfied
                ? "✓ " + t("pa07.dependencies.satisfied")
                : "○ " + t("pa07.dependencies.notSatisfied")
            }}
          </span>
          <Button
            v-if="!isClosed"
            variant="ghost"
            size="sm"
            data-test="dep-remove"
            @click="removeTarget = edge"
          >
            {{ t("pa07.dependencies.removeTitle") }}
          </Button>
        </div>
      </section>

      <section class="td-section">
        <p class="td-section__caption">{{ t("pa07.dependencies.blocks") }}</p>
        <div
          v-if="edges.downstream.length === 0"
          class="td-row td-row--muted"
          data-test="dep-downstream-empty"
        >
          {{ t("pa07.relationships.emptyBlocks") }}
        </div>
        <div
          v-for="edge in edges.downstream"
          :key="edge.id"
          class="td-row"
          data-test="dep-downstream-row"
        >
          <div class="td-row__main">
            <button class="td-row__title" @click="openTask(edge.id)">
              {{ edge.title }}
            </button>
            <span class="td-row__meta">
              {{ aggregateStatusLabel(edge.status) }} ·
              {{ conditionLabel(edge) }}
            </span>
          </div>
          <span
            class="td-row__result"
            :class="{ 'is-satisfied': edge.satisfied }"
          >
            {{
              edge.satisfied
                ? "✓ " + t("pa07.dependencies.satisfied")
                : "○ " + t("pa07.dependencies.notSatisfied")
            }}
          </span>
        </div>
      </section>
    </template>

    <!-- 三步流面板（B2/B3：整卡覆盖式面板，照设计稿两栏） -->
    <div v-if="wizardOpen" class="td-wizard" data-test="dep-wizard">
      <div class="td-wizard__head">
        <p class="td-wizard__title">{{ t("pa07.dependencies.wizardTitle") }}</p>
        <p class="td-wizard__steps">
          <span :class="{ 'is-on': wizardStep === 1 }">{{
            t("pa07.dependencies.wizardStep1")
          }}</span>
          <span :class="{ 'is-on': wizardStep === 2 }">{{
            t("pa07.dependencies.wizardStep2")
          }}</span>
          <span :class="{ 'is-on': wizardStep === 3 }">{{
            t("pa07.dependencies.wizardStep3")
          }}</span>
        </p>
      </div>

      <div v-if="wizardStep === 1" class="td-wizard__body">
        <label class="td-field">
          <span class="td-field__label">{{
            t("pa07.dependencies.wizardPick")
          }}</span>
          <select
            v-model="wizardPredecessor"
            class="td-field__input"
            data-test="wizard-pick"
          >
            <option value="" disabled>
              {{
                wizardPickable.length === 0
                  ? t("pa07.dependencies.wizardNoTasks")
                  : ""
              }}
            </option>
            <option
              v-for="task in wizardPickable"
              :key="task.id"
              :value="task.id"
            >
              {{ task.code }} {{ task.title }}
            </option>
          </select>
        </label>
        <div class="td-wizard__actions">
          <Button variant="secondary" size="md" @click="closeWizard">
            {{ t("common.close") }}
          </Button>
          <Button
            variant="primary"
            size="md"
            :disabled="!wizardPredecessor"
            data-test="wizard-next"
            @click="wizardStep = 2"
          >
            {{ t("pa07.dependencies.wizardNext") }}
          </Button>
        </div>
      </div>

      <div v-else-if="wizardStep === 2" class="td-wizard__body">
        <label class="td-field">
          <span class="td-field__label">{{
            t("pa07.dependencies.wizardCondition")
          }}</span>
          <select
            v-model="wizardCondition"
            class="td-field__input"
            data-test="wizard-condition"
          >
            <option value="completed">
              {{ t("pa07.dependencies.conditionCompleted") }}
            </option>
            <option value="accepted">
              {{ t("pa07.dependencies.conditionAccepted") }}
            </option>
            <option value="deliverable_approved">
              {{ t("pa07.dependencies.conditionDeliverable") }}
            </option>
          </select>
        </label>
        <label
          v-if="wizardCondition === 'deliverable_approved'"
          class="td-field"
        >
          <span class="td-field__label"
            >{{ t("pa07.dependencies.conditionDeliverable") }} (id)</span
          >
          <input
            v-model="wizardDeliverableRef"
            class="td-field__input"
            data-test="wizard-deliverable"
            type="text"
          />
        </label>
        <div class="td-wizard__actions">
          <Button variant="secondary" size="md" @click="wizardStep = 1">
            {{ t("pa07.dependencies.wizardBack") }}
          </Button>
          <Button
            variant="primary"
            size="md"
            data-test="wizard-to-review"
            @click="wizardStep = 3"
          >
            {{ t("pa07.dependencies.wizardNext") }}
          </Button>
        </div>
      </div>

      <div v-else class="td-wizard__body">
        <div class="td-review" data-test="wizard-review">
          <p class="td-review__line">
            {{ wizardPickLabel }} → {{ detail?.task.code }}
            {{ detail?.task.title }}
          </p>
          <p class="td-review__copy">
            {{
              t("pa07.dependencies.wizardReviewCopy", {
                from: wizardPickLabel,
                to: detail?.task.code ?? ""
              })
            }}
          </p>
        </div>
        <p v-if="wizardError" class="td-error" data-test="wizard-error">
          {{ wizardError }}
        </p>
        <div class="td-wizard__actions">
          <Button variant="secondary" size="md" @click="wizardStep = 2">
            {{ t("pa07.dependencies.wizardBack") }}
          </Button>
          <Button
            variant="primary"
            size="md"
            :loading="saving"
            data-test="wizard-save"
            @click="submitWizard"
          >
            {{ t("pa07.dependencies.wizardSave") }}
          </Button>
        </div>
      </div>
    </div>

    <!-- 移除确认（C2） -->
    <div v-if="removeTarget" class="td-remove" data-test="dep-remove-confirm">
      <p class="td-wizard__title">{{ t("pa07.dependencies.removeTitle") }}</p>
      <p class="td-gate__copy">
        {{ t("pa07.dependencies.removeBody", { code: removeTarget.title }) }}
      </p>
      <p v-if="removeError" class="td-error">{{ removeError }}</p>
      <div class="td-wizard__actions">
        <Button variant="secondary" size="md" @click="removeTarget = null">
          {{ t("common.close") }}
        </Button>
        <Button
          variant="danger"
          size="md"
          data-test="dep-remove-confirm-button"
          @click="confirmRemove"
        >
          {{ t("pa07.dependencies.removeConfirm") }}
        </Button>
      </div>
    </div>
  </PageFrame>
</template>

<style scoped>
.td-error {
  margin: 0;
  color: var(--ws-color-danger-text);
}

.td-note {
  margin: 0;
  color: var(--ws-text-secondary);
}

.td-objective {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-base);
}

.td-objective__code {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.td-objective__title {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.td-objective__status {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.td-objective__status--info {
  color: var(--ws-color-info-text);
}

.td-objective__status--success {
  color: var(--ws-color-success-text);
}

.td-objective__status--danger {
  color: var(--ws-color-danger-text);
}

.td-gate {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-raised);
}

.td-gate--satisfied {
  background: var(--surface-base);
}

.td-gate__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.td-gate__copy {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: 1.45;
  color: var(--ws-text-secondary);
}

.td-gate__summary {
  margin: var(--space-2) 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-status-scheduled);
}

.td-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.td-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.td-section__caption {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: 500;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--ws-text-secondary);
}

.td-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-base);
}

.td-row--muted {
  color: var(--ws-text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.td-row__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1;
  min-width: 0;
}

.td-row__title {
  padding: 0;
  border: none;
  background: none;
  text-align: left;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 500;
  color: var(--ws-text-primary);
  cursor: pointer;
}

.td-row__meta {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.td-row__result {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: 500;
  color: var(--ws-text-secondary);
}

.td-row__result.is-satisfied {
  color: var(--ws-color-success-text);
}

.td-wizard,
.td-remove {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-raised);
}

.td-wizard__head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-3);
}

.td-wizard__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.td-wizard__steps {
  display: flex;
  gap: var(--space-4);
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-disabled);
}

.td-wizard__steps .is-on {
  color: var(--ws-color-status-scheduled);
  font-weight: 500;
}

.td-wizard__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.td-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.td-field__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.td-field__input {
  height: var(--size-control-md);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  background: var(--surface-base);
  color: var(--ws-text-primary);
}

.td-wizard__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
}

.td-review__line {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 500;
  color: var(--ws-text-primary);
}

.td-review__copy {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
