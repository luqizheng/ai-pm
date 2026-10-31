<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { AlertTriangle } from "lucide-vue-next";
import PageHeader from "./PageHeader.vue";
import { decisions } from "../fixtures";
import {
  principalNameMap,
  toDecision,
  type PageDecision
} from "./decisions-model";
import { decideApproval, listDecisions, listPrincipals } from "@/api/pm";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import { formatDateTime } from "@/api/format";
import type { Principal } from "@/api/types";
import type { DecisionRecord } from "@/api/workspace-view";

const { t } = useI18n();
const route = useRoute();
const scope = useScopeStore();

/** mock 模式用演示数据；真后端模式读审批请求 + 变更提案。单测一律走 fixtures。 */
const useMock =
  import.meta.env.VITE_USE_MOCK === "1" || import.meta.env.MODE === "test";


const loading = ref(false);
const failed = ref(false);
const records = ref<DecisionRecord[]>([]);
const principals = ref<Principal[]>([]);
const reason = ref("");
const reasonMissing = ref(false);
const submitting = ref(false);
const submitFailed = ref(false);
const actionDone = ref(false);

const projectId = computed<string | undefined>(() => {
  if (scope.projectIdParam) return scope.projectIdParam;
  const param = String(route.params.projectId ?? "");
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(param)
  ) {
    return param;
  }
  return undefined;
});

const selectedProjectId = computed({
  get: () => (scope.currentProjectId === SCOPE_ALL ? "" : scope.currentProjectId),
  set: (value: string) => {
    scope.currentProjectId = value || SCOPE_ALL;
  }
});

const principalNames = computed<Record<string, string>>(() =>
  principalNameMap(principals.value)
);

const pageDecisions = computed<PageDecision[]>(() =>
  useMock
    ? decisions
    : records.value.map(record =>
        toDecision(record, principalNames.value, {
          approvalTitle: String(
            t("workspace.pages.decisionsExtra.approvalTitle")
          ),
          noRationale: String(t("workspace.pages.decisionsExtra.noRationale")),
          requiredRole: role =>
            String(
              t("workspace.pages.decisionsExtra.requiredRoleValue", { role })
            ),
          riskLabel: level =>
            String(t(`workspace.pages.decisionsExtra.risk.${level}`)),
          formatDecidedAt: formatDateTime
        })
      )
);

async function loadDecisions() {
  if (useMock) return;
  failed.value = false;
  if (!projectId.value) {
    records.value = [];
    return;
  }
  loading.value = true;
  try {
    const [list, people] = await Promise.all([
      listDecisions({ project_id: projectId.value, page: 1, size: 200 }),
      listPrincipals({ page: 1, size: 200 })
    ]);
    records.value = list.items ?? [];
    principals.value = people.items ?? [];
  } catch {
    failed.value = true;
    records.value = [];
  } finally {
    loading.value = false;
  }
}

function decideFor(decision: PageDecision, kind: "approved" | "rejected") {
  if (decision.record) void submit(decision.record, kind);
}

async function submit(record: DecisionRecord, kind: "approved" | "rejected") {
  if (reason.value.trim() === "") {
    reasonMissing.value = true;
    return;
  }
  reasonMissing.value = false;
  submitFailed.value = false;
  actionDone.value = false;
  submitting.value = true;
  try {
    await decideApproval(record.id, {
      decision: kind,
      reason: reason.value.trim()
    });
    reason.value = "";
    actionDone.value = true;
    await loadDecisions();
  } catch {
    submitFailed.value = true;
  } finally {
    submitting.value = false;
  }
}

let initialised = false;

onMounted(async () => {
  if (useMock) return;
  try {
    await scope.ensureLoaded();
  } catch {
    failed.value = true;
    return;
  }
  initialised = true;
  await loadDecisions();
});

watch(projectId, () => {
  if (!useMock && initialised) void loadDecisions();
});
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.decisions.title')"
      :subtitle="t('workspace.pages.decisions.subtitle')"
    />

    <div v-if="!useMock" class="ws-toolbar">
      <label class="ws-toolbar-label" for="decisions-project">
        {{ t("workspace.pages.decisionsExtra.project") }}
      </label>
      <select
        id="decisions-project"
        v-model="selectedProjectId"
        class="ws-toolbar-select"
      >
        <option value="">
          {{ t("workspace.pages.decisionsExtra.pickProjectShort") }}
        </option>
        <option v-for="p in scope.projects" :key="p.id" :value="p.id">
          {{ p.name }}
        </option>
      </select>
    </div>
    <p
      v-if="!useMock && !projectId"
      class="ws-hint"
      data-test="decisions-pick-project"
    >
      {{ t("workspace.pages.decisionsExtra.pickProject") }}
    </p>
    <p v-else-if="failed" class="ws-hint" data-test="decisions-failed">
      {{ t("workspace.pages.decisionsExtra.loadFailed") }}
    </p>
    <p
      v-else-if="!useMock && !loading && !pageDecisions.length"
      class="ws-hint"
      data-test="decisions-empty"
    >
      {{ t("workspace.pages.decisionsExtra.empty") }}
    </p>
    <p v-if="actionDone" class="ws-hint is-ok" data-test="decision-done">
      {{ t("workspace.pages.decisionsExtra.submitDone") }}
    </p>
    <p v-if="submitFailed" class="ws-hint is-error" data-test="decision-failed">
      {{ t("workspace.pages.decisionsExtra.submitFailed") }}
    </p>

    <section
      v-for="decision in pageDecisions"
      :key="decision.id"
      class="ws-card ws-decision"
      data-test="decision-card"
    >
      <!-- V1.2：需求变化触发的重新评估提示（系统主动提示） -->
      <div
        v-if="decision.reassess"
        class="ws-reassess"
        data-test="decision-reassess"
        role="alert"
      >
        <AlertTriangle :size="14" aria-hidden="true" />
        <div>
          <strong>{{
            t("workspace.pages.decisionsExtra.reassessTitle")
          }}</strong>
          <span>{{ decision.reassess.reason }}</span>
          <span class="ws-reassess-src">
            {{ decision.reassess.triggeredBy }}
          </span>
        </div>
      </div>

      <div class="ws-decision-head">
        <h2 class="ws-decision-title">{{ decision.title }}</h2>
        <span
          class="ws-badge"
          :class="decision.status === 'pending' ? 'is-pending' : 'is-decided'"
        >
          {{
            t(
              decision.status === "pending"
                ? "workspace.pages.decisions.pending"
                : "workspace.pages.decisions.decided"
            )
          }}
        </span>
      </div>

      <!-- V1.2：问题背景 -->
      <div v-if="decision.background" class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisionsExtra.background") }}
        </span>
        <p class="ws-decision-impact">{{ decision.background }}</p>
      </div>
      <p class="ws-decision-question">{{ decision.question }}</p>

      <div v-if="decision.options.length" class="ws-options">
        <div
          v-for="option in decision.options"
          :key="option.name"
          class="ws-option"
          data-test="decision-option"
        >
          <h3 class="ws-option-name">{{ option.name }}</h3>
          <div class="ws-option-group">
            <span class="ws-option-label is-pros">
              {{ t("workspace.pages.decisions.pros") }}
            </span>
            <ul class="ws-option-list">
              <li v-for="(item, i) in option.pros" :key="i">{{ item }}</li>
            </ul>
          </div>
          <div class="ws-option-group">
            <span class="ws-option-label is-cons">
              {{ t("workspace.pages.decisions.cons") }}
            </span>
            <ul class="ws-option-list">
              <li v-for="(item, i) in option.cons" :key="i">{{ item }}</li>
            </ul>
          </div>
        </div>
      </div>

      <!-- V1.2：比较维度（双方案对照） -->
      <div v-if="decision.dimensions.length" class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisionsExtra.dimensions") }}
        </span>
        <div class="ws-dims" data-test="decision-dimensions">
          <div class="ws-dims-head">
            <span class="ws-dims-name"></span>
            <span
              v-for="option in decision.options"
              :key="option.name"
              class="ws-dims-opt"
            >
              {{ option.name }}
            </span>
          </div>
          <div
            v-for="dim in decision.dimensions"
            :key="dim.name"
            class="ws-dims-row"
          >
            <span class="ws-dims-name">{{ dim.name }}</span>
            <span
              v-for="(value, i) in dim.values"
              :key="i"
              class="ws-dims-value"
            >
              {{ value }}
            </span>
          </div>
        </div>
      </div>

      <div v-if="decision.evidence.length" class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisions.evidence") }}
        </span>
        <ul class="ws-decision-list">
          <li v-for="(item, i) in decision.evidence" :key="i">{{ item }}</li>
        </ul>
      </div>

      <!-- V1.2：AI 推荐理由 + 风险 -->
      <div v-if="decision.recommendation" class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisionsExtra.recommendation") }}
        </span>
        <p class="ws-decision-impact">{{ decision.recommendation }}</p>
      </div>
      <div v-if="decision.risks.length" class="ws-decision-block">
        <span class="ws-decision-label is-risk">
          {{ t("workspace.pages.decisionsExtra.risks") }}
        </span>
        <ul class="ws-decision-list">
          <li v-for="(item, i) in decision.risks" :key="i">{{ item }}</li>
        </ul>
      </div>

      <div v-if="decision.impact" class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisions.impact") }}
        </span>
        <p class="ws-decision-impact">{{ decision.impact }}</p>
      </div>

      <!-- 真后端模式：待决策记录可直接批准/驳回（理由必填） -->
      <div
        v-if="decision.record && decision.status === 'pending'"
        class="ws-decision-actions"
        data-test="decision-actions"
      >
        <input
          v-model="reason"
          class="ws-action-reason"
          type="text"
          :placeholder="t('workspace.pages.decisionsExtra.reasonPlaceholder')"
          :aria-invalid="reasonMissing"
        />
        <button
          type="button"
          class="ws-action-btn is-approve ws-focusable"
          :disabled="submitting"
          data-test="decision-approve"
          @click="decideFor(decision, 'approved')"
        >
          {{ t("workspace.pages.decisionsExtra.approve") }}
        </button>
        <button
          type="button"
          class="ws-action-btn is-reject ws-focusable"
          :disabled="submitting"
          data-test="decision-reject"
          @click="decideFor(decision, 'rejected')"
        >
          {{ t("workspace.pages.decisionsExtra.reject") }}
        </button>
      </div>
      <p v-if="reasonMissing" class="ws-hint is-error">
        {{ t("workspace.pages.decisionsExtra.reasonRequired") }}
      </p>

      <!-- V1.2：决策人 / 批准时间 / 后续验证 -->
      <div v-if="decision.status === 'decided'" class="ws-decision-meta">
        <span>
          {{ t("workspace.pages.decisionsExtra.decidedBy") }}：{{
            decision.decidedBy
          }}
        </span>
        <span>
          {{ t("workspace.pages.decisionsExtra.approvedAt") }}：{{
            decision.approvedAt
          }}
        </span>
      </div>
      <div v-if="decision.conclusion" class="ws-decision-block">
        <span class="ws-decision-label is-conclusion">
          {{ t("workspace.pages.decisions.conclusion") }}
        </span>
        <p class="ws-decision-impact">{{ decision.conclusion }}</p>
      </div>
      <div v-if="decision.verification?.length" class="ws-decision-block">
        <span class="ws-decision-label">
          {{ t("workspace.pages.decisionsExtra.verification") }}
        </span>
        <ul class="ws-decision-list">
          <li v-for="(item, i) in decision.verification" :key="i">
            {{ item }}
          </li>
        </ul>
      </div>
    </section>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px 24px 32px;
  max-width: 960px;
}

.ws-decision {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
}

.ws-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-toolbar-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-toolbar-select {
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-surface);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
}

.ws-hint {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-hint.is-ok {
  color: var(--ws-color-success-text);
}

.ws-hint.is-error {
  color: var(--ws-color-danger-text);
}

.ws-decision-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-action-reason {
  flex: 1;
  min-width: 0;
  height: 30px;
  padding: 0 8px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-surface);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
}

.ws-action-reason[aria-invalid="true"] {
  border-color: var(--ws-color-danger-text);
}

.ws-action-btn {
  flex: none;
  height: 30px;
  padding: 0 12px;
  border-radius: var(--ws-radius-card);
  border: 1px solid var(--ws-border);
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.ws-action-btn.is-approve {
  color: var(--ws-color-success-text);
}

.ws-action-btn.is-reject {
  color: var(--ws-color-danger-text);
}

.ws-action-btn:disabled {
  opacity: 0.6;
  cursor: default;
}

.ws-reassess {
  display: flex;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid currentcolor;
  border-radius: var(--ws-radius-card);
  color: var(--ws-color-warning-text);
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}

.ws-reassess strong {
  margin-right: 6px;
}

.ws-reassess-src {
  margin-left: 8px;
  color: var(--ws-text-secondary);
}

.ws-decision-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.ws-decision-title {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 紫色克制：状态用语义色 */
.ws-badge.is-pending {
  color: var(--ws-color-warning-text);
}

.ws-badge.is-decided {
  color: var(--ws-color-success-text);
}

.ws-decision-question {
  margin: 0;
  font-size: var(--ws-font-size-body);
}

.ws-options {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.ws-option {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-option-name {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: 600;
}

.ws-option-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ws-option-label {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
}

.ws-option-label.is-pros {
  color: var(--ws-color-success-text);
}

.ws-option-label.is-cons {
  color: var(--ws-color-danger-text);
}

.ws-option-list {
  margin: 0;
  padding-left: 16px;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}

/* 比较维度对照表 */
.ws-dims {
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow: hidden;
}

.ws-dims-head,
.ws-dims-row {
  display: grid;
  grid-template-columns: 120px repeat(auto-fit, minmax(0, 1fr));
}

.ws-dims-head {
  background: var(--ws-elevated);
}

.ws-dims-head > span,
.ws-dims-row > span {
  padding: 6px 10px;
  font-size: var(--ws-font-size-caption);
  border-bottom: 1px solid var(--ws-border);
}

.ws-dims-row:last-child > span {
  border-bottom: none;
}

.ws-dims-name {
  font-weight: 600;
}

.ws-dims-value {
  color: var(--ws-text-primary);
}

.ws-decision-block {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ws-decision-label {
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-decision-label.is-conclusion {
  color: var(--ws-color-success-text);
}

.ws-decision-label.is-risk {
  color: var(--ws-color-danger-text);
}

.ws-decision-list {
  margin: 0;
  padding-left: 16px;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}

.ws-decision-impact {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  line-height: 1.6;
}

.ws-decision-meta {
  display: flex;
  gap: 16px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
