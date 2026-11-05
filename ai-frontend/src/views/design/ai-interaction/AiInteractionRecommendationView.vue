<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { recommendationFixture } from "@/components/ui/ai-interaction/fixtures";
import RecommendationResponse from "@/components/ui/ai-interaction/responses/RecommendationResponse.vue";
import { createMockSubmit } from "@/components/ui/ai-interaction/submitAdapter";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 帧 03/04/05 — 建议卡三态。
 *
 * 页面级口径：AI 建议 = accent 色；「接收」只记录选择，不等于审批。
 * Pending 帧真实可交互（接收 / 拒绝 + 拒因）；Accepted / Rejected 帧
 * 用 initialStatus 呈现只读结果，并补上卡片之外仍需可见的记录号与拒因。
 */
const { t } = useI18n();

/** 页面级提交适配器：300ms 延迟用于展示「提交中」，结果只写「已记录」 */
const submit = createMockSubmit({ latencyMs: 300 });

/** Accepted 帧的只读记录号（样例数据；真实页面由提交结果回填） */
const acceptedReference = "REC-2041";

/** Rejected 帧的只读拒绝原因（设计稿英文样板；真实页面由 reject 表单回填） */
const rejectedReasonSample =
  "Terrain interface is not frozen yet — revisit the split after REQ-128 v1.4 is approved.";
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.recommendation.title"
    subtitle-key="design.aiInteraction.recommendation.subtitle"
  >
    <section class="reco-page__legend">
      <p class="reco-page__legend-item">
        <span class="reco-page__swatch" aria-hidden="true" />
        <span class="reco-page__legend-label">{{
          t("ai.controls.decision.recommended")
        }}</span>
        <span class="reco-page__legend-text">
          AI suggestions always use the accent colour; the human decision stays
          in secondary text.
        </span>
      </p>
      <p class="reco-page__legend-item">
        <span class="reco-page__legend-label">{{
          t("ai.recommendation.accepted.note")
        }}</span>
        <span class="reco-page__legend-text">
          Accept records the choice only — accepting is never an approval, and
          no business data changes until the formal flow records it.
        </span>
      </p>
    </section>

    <DesignFrame
      label="03 — Recommendation / Pending"
      width="default"
      note="Pending: accept records the choice; reject expands a reason field. Both outcomes are interaction records, not approvals."
    >
      <RecommendationResponse
        :payload="recommendationFixture"
        :submit="submit"
        timestamp="10:01"
      />
    </DesignFrame>

    <DesignFrame
      label="04 — Recommendation / Accepted (read-only)"
      width="default"
      note="Accepted: read-only summary + recorded reference. The card states explicitly that nothing has taken effect yet."
    >
      <RecommendationResponse
        :payload="recommendationFixture"
        initial-status="accepted"
        timestamp="10:06"
      />
      <p class="reco-page__record">
        <code class="reco-page__record-reference">{{
          t("ai.card.submission.reference", { reference: acceptedReference })
        }}</code>
      </p>
    </DesignFrame>

    <DesignFrame
      label="05 — Recommendation / Rejected (read-only)"
      width="default"
      note="Rejected: the read-only summary stays on screen and the reason captured by the reject form is kept as plain text."
    >
      <RecommendationResponse
        :payload="recommendationFixture"
        initial-status="rejected"
        timestamp="10:08"
      />
      <div class="reco-page__reason">
        <p class="reco-page__reason-label">
          {{ t("ai.recommendation.rejected.reason") }}
        </p>
        <p class="reco-page__reason-text">{{ rejectedReasonSample }}</p>
      </div>
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.reco-page__legend {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.reco-page__legend-item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
}

.reco-page__swatch {
  flex: none;
  width: var(--space-2);
  height: var(--space-2);
  background: var(--ws-accent);
  border-radius: 50%;
}

.reco-page__legend-label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
}

.reco-page__legend-text {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.reco-page__record {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: var(--space-3) 0 0;
  padding: var(--space-2) var(--space-3);
  background: var(--ws-elevated);
  border-left: var(--size-indicator) solid var(--ws-color-success);
  border-radius: var(--radius-xs);
}

.reco-page__record-reference {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.reco-page__reason {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: var(--space-3) 0 0;
  padding: var(--space-2) var(--space-3);
  background: var(--ws-elevated);
  border-left: var(--size-indicator) solid var(--ws-color-danger);
  border-radius: var(--radius-xs);
}

.reco-page__reason-label {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.reco-page__reason-text {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

@media (prefers-reduced-motion: reduce) {
  .reco-page__legend-item {
    transition: none;
  }
}
</style>
