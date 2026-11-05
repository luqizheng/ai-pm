<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ClipboardCheck } from "lucide-vue-next";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { PageFrame } from "@/components/shell";

import ComparisonCard from "./components/ComparisonCard.vue";
import DecisionFormSheet from "./components/DecisionFormSheet.vue";
import DecisionPanel from "./components/DecisionPanel.vue";
import ReviewDetail from "./components/ReviewDetail.vue";
import ReviewQueue from "./components/ReviewQueue.vue";

import { reviewProposalsFixture, reviewReviewerFixture } from "./fixtures";
import type {
  DecisionKind,
  ReviewFilterKey,
  ReviewProposal
} from "./review-contract";

/**
 * 需求评审与确认页（S02-UI-03）。
 *
 * 两栏 35/65：左 = 评审队列（五档过滤 + 待评审计数），右 = 评审详情
 * （提案需求 / 来源证据 / AI 分析 / 与既有需求并排比较）+ 人工决策面板。
 * 三个决策弹层全部显式：Confirm = ConfirmationDialog（确认摘要 + 我已审阅勾选）；
 * Request Changes / Reject = 表单抽屉（评论/理由必填）。
 * 红线：AI 只产提案（来源徽标可辨），决策记录只能由人类产生；提案评审状态
 * 与需求 Revision 生命周期相互独立（见 review-contract.ts 注释）。
 */
const { t } = useI18n();

const proposals = ref<ReviewProposal[]>(
  structuredClone(reviewProposalsFixture) as ReviewProposal[]
);

const selectedId = ref(reviewProposalsFixture[0]?.id ?? "");
const filter = ref<ReviewFilterKey>("all");

const confirmDialogOpen = ref(false);
const decisionSheetOpen = ref(false);
const decisionMode = ref<"request_changes" | "reject">("request_changes");

const selectedProposal = computed(
  () => proposals.value.find(item => item.id === selectedId.value) ?? null
);

const counts = computed(() => {
  const result: Record<ReviewFilterKey | "all", number> = {
    all: proposals.value.length,
    pending_review: 0,
    changes_requested: 0,
    confirmed: 0,
    rejected: 0
  };
  for (const proposal of proposals.value) {
    result[proposal.status] += 1;
  }
  return result;
});

const filteredProposals = computed(() =>
  filter.value === "all"
    ? proposals.value
    : proposals.value.filter(proposal => proposal.status === filter.value)
);

function nowLabel(): string {
  return new Date().toISOString().slice(0, 16).replace("T", " ");
}

function onSelect(id: string) {
  selectedId.value = id;
}

function confirmSummary(): string {
  const proposal = selectedProposal.value;
  if (!proposal) return "";
  return proposal.requirement_code
    ? t("pm.requirementReview.confirmRevisionSummary", {
        code: proposal.code,
        target: proposal.requirement_code
      })
    : t("pm.requirementReview.confirmNewSummary", {
        code: proposal.code,
        count: proposal.acceptance_criteria.length
      });
}

function landDecision(kind: DecisionKind, comment: string) {
  const proposal = selectedProposal.value;
  if (!proposal) return;
  proposal.status =
    kind === "confirmed"
      ? "confirmed"
      : kind === "changes_requested"
        ? "changes_requested"
        : "rejected";
  proposal.decision = {
    kind,
    reviewer: reviewReviewerFixture,
    decided_at: nowLabel(),
    comment
  };
}

function onConfirmDecision() {
  landDecision("confirmed", "");
  confirmDialogOpen.value = false;
}

function onDecisionSubmit(comment: string) {
  landDecision(
    decisionMode.value === "request_changes" ? "changes_requested" : "rejected",
    comment
  );
  decisionSheetOpen.value = false;
}
</script>

<template>
  <PageFrame
    :title="t('pm.requirementReview.pageTitle')"
    :description="t('pm.requirementReview.pageSubtitle')"
  >
    <template #actions>
      <span class="req-review__origin">
        <ClipboardCheck aria-hidden="true" />
        {{ t("pm.requirementReview.originHint") }}
      </span>
    </template>

    <div class="req-review">
      <ReviewQueue
        :proposals="filteredProposals"
        :selected-id="selectedId"
        :filter="filter"
        :counts="counts"
        @select="onSelect"
        @update:filter="filter = $event"
      />
      <ReviewDetail
        :proposal="selectedProposal"
        @confirm="confirmDialogOpen = true"
        @changes="
          decisionMode = 'request_changes';
          decisionSheetOpen = true;
        "
        @reject="
          decisionMode = 'reject';
          decisionSheetOpen = true;
        "
      />
    </div>

    <ConfirmationDialog
      :open="confirmDialogOpen"
      :title="t('pm.requirementReview.confirmModalTitle')"
      :description="confirmSummary()"
      :confirm-label="t('pm.requirementReview.actionConfirm')"
      :cancel-label="t('common.cancel')"
      :checkbox-label="t('pm.requirementReview.confirmCheckbox')"
      @update:open="value => !value && (confirmDialogOpen = false)"
      @confirm="onConfirmDecision"
      @cancel="confirmDialogOpen = false"
    />

    <DecisionFormSheet
      v-model:open="decisionSheetOpen"
      :mode="decisionMode"
      :proposal-code="selectedProposal?.code ?? ''"
      @submit="onDecisionSubmit"
    />
  </PageFrame>
</template>

<style scoped>
.req-review__origin {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-review__origin svg {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

.req-review {
  display: grid;
  grid-template-columns: 35% minmax(0, 1fr);
  gap: var(--space-4);
  align-items: start;
  min-width: 0;
}

@media (max-width: 1100px) {
  .req-review {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
