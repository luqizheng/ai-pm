<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import Button from "@/components/ui/button/Button.vue";
import type { ActionItem } from "@/api/command-center";
import { reviewItemKey, reviewTabOf } from "@/api/reviewCenter";
import {
  useReviewCenterStore,
  REVIEW_CENTER_PATH,
  type ReviewSort,
  type ReviewTab,
  type FindingDecisionStatus,
  type ProposalDecisionAction,
  type DeliveryDecision,
  type DecisionResult
} from "@/stores/reviewCenter";
import { useShellStore } from "@/stores/shell";

import ReviewTypeTabs from "./components/ReviewTypeTabs.vue";
import ReviewFilterBar from "./components/ReviewFilterBar.vue";
import ReviewQueueItem from "./components/ReviewQueueItem.vue";
import ReviewEmptyState from "./components/ReviewEmptyState.vue";
import type { ReviewEmptyKind } from "./components/ReviewEmptyState.vue";
import ReviewDetailDrawer from "./components/ReviewDetailDrawer.vue";
import ReviewDecisionReasonInput from "./components/ReviewDecisionReasonInput.vue";
import FindingReviewContext from "./components/FindingReviewContext.vue";
import ProposalReviewContext from "./components/ProposalReviewContext.vue";
import DeliveryReviewContext from "./components/DeliveryReviewContext.vue";

/**
 * 人工审核中心（R01–R12，docs/ui/review-center）。
 *
 * 页面容器：路由/数据/状态管理；展示件文案全部经 props 传入（D8）。
 * URL 语义（V1.1 §1.1）：`?kind=`（页签键，兼容实际 kind 值）+
 * `?item=`（kind:id 复合键）；刷新/分享可恢复，Drawer 关闭清 item 留 kind。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const store = useReviewCenterStore();

function currentProjectId(): string {
  const raw = route.query.project_id;
  const value = Array.isArray(raw) ? (raw[0] ?? "") : raw;
  return String(value ?? "");
}

// ── 时间展示 ─────────────────────────────────────────────────────────

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

function timeLabel(iso?: string): string {
  if (!iso) return "";
  const ts = Date.parse(iso);
  if (Number.isNaN(ts)) return "";
  const minutes = Math.floor((Date.now() - ts) / 60000);
  if (minutes < 1) return t("review.time.justNow");
  if (minutes < 60) return t("review.time.minutes", { n: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("review.time.hours", { n: hours });
  return t("review.time.days", { n: Math.floor(hours / 24) });
}

function clockLabel(iso?: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

// ── 页签与筛选 ───────────────────────────────────────────────────────

const tabs = computed(() => {
  const countBy = (tab: "finding" | "proposal" | "delivery") =>
    store.items.filter(item => reviewTabOf(item) === tab).length;
  return [
    { key: "all", label: t("review.tabs.all"), count: store.items.length },
    {
      key: "finding",
      label: t("review.tabs.finding"),
      count: countBy("finding")
    },
    {
      key: "proposal",
      label: t("review.tabs.proposal"),
      count: countBy("proposal")
    },
    {
      key: "delivery",
      label: t("review.tabs.delivery"),
      count: countBy("delivery")
    }
  ];
});

const domainOptions = computed(() => {
  const values = new Set(store.items.map(item => item.domain));
  return [
    { value: "", label: t("review.filter.domainAll") },
    ...[...values].map(value => ({
      value,
      label: t(`review.domain.${value}`)
    }))
  ];
});

const sortOptions = computed(() => [
  { value: "updated_desc", label: t("review.filter.sortUpdatedDesc") },
  { value: "updated_asc", label: t("review.filter.sortUpdatedAsc") }
]);

const emptyKind = computed<ReviewEmptyKind>(() => {
  if (store.status === "loading" || store.status === "idle") return "loading";
  if (store.status === "error") return "loadFailed";
  if (store.filteredItems.length === 0) {
    const filtered =
      store.kindTab !== "all" ||
      store.domainFilter !== null ||
      store.search.trim() !== "";
    return filtered ? "noMatch" : "allDone";
  }
  return "allDone";
});

// ── URL 同步（kind = 页签键，兼容实际 kind 值；item = kind:id 复合键）──

const KIND_PARAM_TO_TAB: Record<string, ReviewTab> = {
  all: "all",
  finding: "finding",
  proposal: "proposal",
  delivery: "delivery",
  finding_pending: "finding",
  finding_revision_requested: "finding",
  proposal_pending: "proposal",
  delivery_pending: "delivery"
};

function syncFromQuery(): void {
  const kindRaw = route.query.kind;
  const kind = String((Array.isArray(kindRaw) ? kindRaw[0] : kindRaw) ?? "");
  if (kind && KIND_PARAM_TO_TAB[kind]) {
    store.kindTab = KIND_PARAM_TO_TAB[kind];
  }
  const itemRaw = route.query.item;
  const item = String((Array.isArray(itemRaw) ? itemRaw[0] : itemRaw) ?? "");
  if (item) store.select(item);
}

function syncToQuery(): void {
  void router.replace({
    query: {
      ...route.query,
      kind: store.kindTab === "all" ? undefined : store.kindTab,
      item: store.selectedKey ?? undefined
    }
  });
}

watch(
  () => store.kindTab,
  () => syncToQuery()
);
watch(
  () => store.selectedKey,
  () => syncToQuery()
);

onMounted(() => {
  syncFromQuery();
  const pid = currentProjectId();
  if (pid) void store.load(pid);
});

watch(
  () => route.query.project_id,
  () => {
    const pid = currentProjectId();
    if (pid && pid !== store.projectId) void store.load(pid);
  }
);

// ── 选中与领域上下文 ─────────────────────────────────────────────────

const selectedItem = computed(() => store.selectedItem);
const selectedKey = computed(() =>
  selectedItem.value ? reviewItemKey(selectedItem.value) : null
);
const selectedTab = computed(() =>
  selectedItem.value ? reviewTabOf(selectedItem.value) : null
);
const isFinding = computed(() => selectedTab.value === "finding");
const isProposal = computed(() => selectedTab.value === "proposal");
const isDelivery = computed(() => selectedTab.value === "delivery");
const busy = computed(
  () => store.selectedKey !== null && store.submittingKey === store.selectedKey
);

const findingCtx = computed(() =>
  selectedKey.value ? store.findingCtx[selectedKey.value] : undefined
);
const proposalCtx = computed(() =>
  selectedKey.value ? store.proposalCtx[selectedKey.value] : undefined
);
const proposalStale = computed(() =>
  selectedKey.value ? (store.proposalStale[selectedKey.value] ?? false) : false
);
const deliveryCtx = computed(() =>
  selectedKey.value ? store.deliveryCtx[selectedKey.value] : undefined
);

/** Drawer 右侧让位：AI Chat 常驻列开启时让出 446px（V1.1 §2.3）。 */
const rightInset = computed(() =>
  shell.aiPanelOpen ? "var(--ws-chat-width)" : "0px"
);

// 决策区本地状态（换选重置）
const reason = ref("");
const armedAction = ref<ProposalDecisionAction | DeliveryDecision | null>(null);
const decisionError = ref<string | null>(null);

function resetDecisionState(): void {
  reason.value = "";
  armedAction.value = null;
  decisionError.value = null;
}

watch(selectedKey, () => resetDecisionState());

watch(selectedItem, item => {
  const pid = currentProjectId();
  if (!item || !pid) return;
  const tab = reviewTabOf(item);
  if (tab === "finding") void store.loadFindingContext(pid, item);
  else if (tab === "proposal") void store.loadProposalContext(pid, item);
  else if (tab === "delivery") void store.loadDeliveryContext(pid, item);
});

// ── 决策提交 ─────────────────────────────────────────────────────────

function nextItemKey(): string | null {
  const list = store.filteredItems;
  const index = list.findIndex(
    item => reviewItemKey(item) === store.selectedKey
  );
  if (index === -1) return null;
  const next = list[index + 1] ?? list[index - 1] ?? null;
  return next ? reviewItemKey(next) : null;
}

/** D5 409 家族 → 领域语义文案（R07：不自动重试）。 */
function conflictMessage(result: DecisionResult, item: ActionItem): string {
  if (selectedTab.value === "delivery") {
    const code = d5ConflictCode(result);
    if (code) return t(`review.delivery.${code}`);
  }
  if (selectedTab.value === "proposal") {
    return t("review.proposal.staleNotice");
  }
  void item;
  return t("review.detail.stateChanged");
}

function d5ConflictCode(result: DecisionResult): string | null {
  // decideReviewApproval 的 409 语义由后端错误码区分（pm_reviews.rs §头部表）
  const raw = result.message ?? "";
  if (raw.includes("already_decided")) return "alreadyDecided";
  if (raw.includes("anchor_drift")) return "anchorDrift";
  if (raw.includes("completion_gate_refused")) return "gateRefused";
  if (raw.includes("review_not_succeeded")) return "reviewNotSucceeded";
  return null;
}

function handleResult(result: DecisionResult, nextKey: string | null): void {
  if (result.ok) {
    resetDecisionState();
    if (nextKey && store.items.some(item => reviewItemKey(item) === nextKey)) {
      store.select(nextKey);
    } else {
      store.select(null);
    }
    return;
  }
  armedAction.value = null;
  if (result.noPermission) {
    decisionError.value = t("review.detail.noPermission");
  } else if (result.conflict) {
    decisionError.value = selectedItem.value
      ? conflictMessage(result, selectedItem.value)
      : t("review.detail.stateChanged");
  } else {
    decisionError.value = result.message ?? t("review.detail.submitFailed");
  }
}

function requireSelection(): ActionItem | null {
  return store.selectedItem;
}

async function submitFinding(status: FindingDecisionStatus): Promise<void> {
  const item = requireSelection();
  if (!item || reason.value.trim() === "") return;
  const nextKey = nextItemKey();
  const result = await store.submitFindingDecision(
    currentProjectId(),
    item,
    status,
    reason.value.trim()
  );
  handleResult(result, nextKey);
}

/** Proposal / Delivery 二次确认（V1.0 §5.2：知识提案误操作成本高）。 */
function armOrSubmit(
  action: ProposalDecisionAction | DeliveryDecision,
  submit: (action: ProposalDecisionAction | DeliveryDecision) => Promise<void>
): void {
  if (armedAction.value !== action) {
    armedAction.value = action;
    return;
  }
  void submit(action);
}

async function submitProposal(action: ProposalDecisionAction): Promise<void> {
  const item = requireSelection();
  if (!item) return;
  const nextKey = nextItemKey();
  const result = await store.submitProposalDecision(
    currentProjectId(),
    item,
    action,
    reason.value
  );
  handleResult(result, nextKey);
}

async function submitDelivery(decision: DeliveryDecision): Promise<void> {
  const item = requireSelection();
  if (!item) return;
  const nextKey = nextItemKey();
  const result = await store.submitDeliveryDecision(
    currentProjectId(),
    item,
    decision,
    reason.value
  );
  handleResult(result, nextKey);
}

function onViewObject(): void {
  const item = selectedItem.value;
  if (!item) return;
  const tab = reviewTabOf(item);
  if (tab === "delivery" && item.link.params.task_id) {
    void router.push(`/project/tasks/${item.link.params.task_id}`);
    return;
  }
  if (tab === "proposal") void router.push("/project/knowledge");
  else if (tab === "finding") void router.push("/project/discovery");
}

const viewObjectLabel = computed(() =>
  isDelivery.value
    ? t("review.delivery.viewTask")
    : t("review.detail.viewObject")
);

const stateLabelOf = (item: ActionItem): string => {
  const key = `review.sourceState.${item.source_state}`;
  const label = t(key);
  return label === key ? item.source_state : label;
};

const kindLabelOf = (item: ActionItem): string => t(`review.kind.${item.kind}`);

const kindToneOf = (
  item: ActionItem
): "neutral" | "accent" | "warning" | "danger" | "success" | "info" => {
  if (item.kind === "finding_pending") return "warning";
  if (item.kind === "finding_revision_requested") return "danger";
  if (item.kind === "proposal_pending") return "info";
  if (item.kind === "delivery_pending") return "accent";
  return "neutral";
};

function retryLoad(): void {
  const pid = currentProjectId();
  if (pid) void store.load(pid);
}
</script>

<template>
  <div class="review-page" :data-path="REVIEW_CENTER_PATH">
    <header class="review-page__head">
      <h1 class="review-page__title">{{ t("review.page.title") }}</h1>
      <p class="review-page__subtitle">{{ t("review.page.subtitle") }}</p>
      <p
        v-if="store.totalCount !== null"
        class="review-page__meta"
        data-testid="review-meta"
      >
        {{
          t("review.page.meta", {
            total: store.totalCount,
            asOf: clockLabel(store.block?.asOf)
          })
        }}
      </p>
    </header>

    <ReviewTypeTabs
      :tabs="tabs"
      :model-value="store.kindTab"
      @update:model-value="store.kindTab = $event as ReviewTab"
    />

    <ReviewFilterBar
      :model-search="store.search"
      :search-placeholder="t('review.filter.searchPlaceholder')"
      :domains="domainOptions"
      :model-domain="store.domainFilter ?? ''"
      :sort-options="sortOptions"
      :model-sort="store.sortBy"
      @update:model-search="store.search = $event"
      @update:model-domain="store.domainFilter = $event || null"
      @update:model-sort="store.sortBy = $event as ReviewSort"
    />

    <div class="review-page__queue">
      <ReviewEmptyState
        v-if="store.filteredItems.length === 0"
        :kind="emptyKind"
        :title="t(`review.empty.${emptyKind}.title`)"
        :hint="t(`review.empty.${emptyKind}.hint`)"
        :retry-label="t('review.empty.loadFailed.retry')"
        @retry="retryLoad"
      />
      <template v-else>
        <ReviewQueueItem
          v-for="item in store.filteredItems"
          :key="reviewItemKey(item)"
          :kind-label="kindLabelOf(item)"
          :kind-tone="kindToneOf(item)"
          :state-label="stateLabelOf(item)"
          :title="item.title"
          :detail="item.detail ?? ''"
          :domain-label="t(`review.domain.${item.domain}`)"
          :time-label="timeLabel(item.updated_at)"
          :selected="reviewItemKey(item) === store.selectedKey"
          @select="store.select(reviewItemKey(item))"
        />
      </template>
    </div>

    <ReviewDetailDrawer
      :open="selectedItem !== null"
      :title="selectedItem?.title ?? ''"
      :kind-label="selectedItem ? kindLabelOf(selectedItem) : ''"
      :state-label="selectedItem ? stateLabelOf(selectedItem) : ''"
      :time-label="selectedItem ? timeLabel(selectedItem.updated_at) : ''"
      :hint="selectedItem?.action_hint ?? ''"
      :close-label="t('review.detail.close')"
      :right-inset="rightInset"
      :busy="busy"
      @close="store.select(null)"
    >
      <template #context>
        <FindingReviewContext
          v-if="isFinding"
          :finding="findingCtx?.data ?? null"
          :loading="findingCtx?.loading ?? false"
          :failed="findingCtx?.failed ?? false"
          :context-title="t('review.finding.contextTitle')"
          :loading-label="t('review.detail.contextLoading')"
          :failed-label="t('review.detail.contextFailed')"
          :kind-label="t('review.finding.kindLabel')"
          :confidence-label="t('review.finding.confidenceLabel')"
          :evidence-label="t('review.finding.evidenceLabel')"
          :no-evidence-label="t('review.finding.noEvidence')"
        />
        <ProposalReviewContext
          v-else-if="isProposal"
          :proposal="proposalCtx?.data ?? null"
          :loading="proposalCtx?.loading ?? false"
          :failed="proposalCtx?.failed ?? false"
          :stale="proposalStale"
          :context-title="t('review.proposal.contextTitle')"
          :loading-label="t('review.detail.contextLoading')"
          :failed-label="t('review.detail.contextFailed')"
          :rationale-label="t('review.proposal.rationaleLabel')"
          :risk-label="t('review.proposal.riskLabel')"
          :confidence-label="t('review.proposal.confidenceLabel')"
          :governance-label="t('review.proposal.governanceLabel')"
          :member-count-label="t('review.proposal.memberCountLabel')"
          :version-label="t('review.proposal.versionLabel')"
          :stale-notice="t('review.proposal.staleNotice')"
        />
        <DeliveryReviewContext
          v-else-if="isDelivery"
          :context="deliveryCtx?.data ?? null"
          :loading="deliveryCtx?.loading ?? false"
          :failed="deliveryCtx?.failed ?? false"
          :context-title="t('review.delivery.contextTitle')"
          :loading-label="t('review.detail.contextLoading')"
          :failed-label="t('review.detail.contextFailed')"
          :repository-label="t('review.delivery.repositoryLabel')"
          :delivery-state-label="t('review.delivery.deliveryStateLabel')"
          :commit-label="t('review.delivery.commitLabel')"
          :changed-files-label="t('review.delivery.changedFilesLabel')"
          :verdicts-label="t('review.delivery.verdictsLabel')"
          :verdict-not-evaluated-label="
            t('review.delivery.verdictNotEvaluated')
          "
          :no-evidence-label="t('review.delivery.noEvidence')"
          :pending-notice="t('review.delivery.pendingNotice')"
          :no-pending-notice="t('review.delivery.noPendingNotice')"
        />
      </template>

      <template #decision>
        <section
          v-if="selectedItem"
          class="review-decision"
          :data-busy="busy"
          aria-label="decision"
        >
          <h3 class="review-decision__title">
            {{ t("review.detail.decisionLabel") }}
          </h3>

          <p
            v-if="store.lastFeedback"
            class="review-decision__feedback"
            role="status"
          >
            {{ t(store.lastFeedback) }}
          </p>
          <p v-if="decisionError" class="review-decision__error" role="alert">
            {{ decisionError }}
          </p>

          <ReviewDecisionReasonInput
            v-if="isFinding"
            v-model="reason"
            :label="t('review.reason.findingLabel')"
            :placeholder="t('review.reason.findingPlaceholder')"
            :required="true"
            :required-badge-label="t('review.reason.requiredBadge')"
            :optional-badge-label="t('review.reason.optionalBadge')"
            :required-hint="t('review.reason.requiredHint')"
            :disabled="busy"
          />
          <div v-if="isFinding" class="review-decision__actions">
            <Button
              variant="secondary"
              :disabled="busy || reason.trim() === ''"
              @click="submitFinding('REJECTED')"
            >
              {{ t("review.finding.reject") }}
            </Button>
            <Button
              :disabled="busy || reason.trim() === ''"
              @click="submitFinding('CONFIRMED')"
            >
              {{ t("review.finding.confirm") }}
            </Button>
          </div>

          <template v-else-if="isProposal">
            <ReviewDecisionReasonInput
              v-model="reason"
              :label="t('review.reason.proposalLabel')"
              :placeholder="t('review.reason.proposalPlaceholder')"
              :required="false"
              :required-badge-label="t('review.reason.requiredBadge')"
              :optional-badge-label="t('review.reason.optionalBadge')"
              :required-hint="''"
              :disabled="busy"
            />
            <div class="review-decision__actions">
              <Button
                variant="secondary"
                :disabled="busy"
                @click="armOrSubmit('reject', submitProposal)"
              >
                {{
                  armedAction === "reject"
                    ? t("review.proposal.rejectArmed")
                    : t("review.proposal.reject")
                }}
              </Button>
              <Button
                :disabled="busy"
                @click="armOrSubmit('approve', submitProposal)"
              >
                {{
                  armedAction === "approve"
                    ? t("review.proposal.confirmArmed")
                    : t("review.proposal.approve")
                }}
              </Button>
            </div>
          </template>

          <template v-else-if="isDelivery">
            <ReviewDecisionReasonInput
              v-model="reason"
              :label="t('review.delivery.reasonLabel')"
              :placeholder="t('review.delivery.reasonPlaceholder')"
              :required="false"
              :required-badge-label="t('review.reason.requiredBadge')"
              :optional-badge-label="t('review.reason.optionalBadge')"
              :required-hint="''"
              :disabled="busy || !deliveryCtx?.data?.pendingApproval"
            />
            <div class="review-decision__actions">
              <Button
                variant="secondary"
                :disabled="busy || !deliveryCtx?.data?.pendingApproval"
                @click="armOrSubmit('rejected', submitDelivery)"
              >
                {{
                  armedAction === "rejected"
                    ? t("review.proposal.rejectArmed")
                    : t("review.delivery.rejectAction")
                }}
              </Button>
              <Button
                :disabled="busy || !deliveryCtx?.data?.pendingApproval"
                @click="armOrSubmit('approved', submitDelivery)"
              >
                {{
                  armedAction === "approved"
                    ? t("review.proposal.confirmArmed")
                    : t("review.delivery.approveAction")
                }}
              </Button>
            </div>
            <Button variant="secondary" size="sm" @click="onViewObject">
              {{ viewObjectLabel }}
            </Button>
          </template>
        </section>
      </template>
    </ReviewDetailDrawer>
  </div>
</template>

<style scoped>
.review-page {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  min-height: 0;
  padding: 16px;
  overflow-y: auto;
}

.review-page__head {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.review-page__title {
  margin: 0;
  color: var(--text-primary);
  font-size: 18px;
  font-weight: 600;
}

.review-page__subtitle {
  margin: 0;
  color: var(--text-secondary);
  font-size: 13px;
}

.review-page__meta {
  margin: 0;
  color: var(--text-secondary);
  font-size: 12px;
}

.review-page__queue {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.review-decision {
  border-top: 1px solid var(--border-subtle);
  padding-top: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.review-decision__title {
  margin: 0;
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 600;
}

.review-decision__feedback {
  margin: 0;
  padding: 8px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  color: var(--text-primary);
  font-size: 13px;
}

.review-decision__error {
  margin: 0;
  padding: 8px 12px;
  border: 1px solid var(--border-default);
  border-radius: 8px;
  color: var(--text-primary);
  font-size: 13px;
}

.review-decision__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
