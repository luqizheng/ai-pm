<script setup lang="ts">
import { useI18n } from "vue-i18n";

import Badge from "@/components/ui/badge/Badge.vue";
import AiSubmissionStatus from "@/components/ui/ai-interaction/card/AiSubmissionStatus.vue";
import {
  proposalDiffFixture,
  proposalFixture
} from "@/components/ui/ai-interaction/fixtures";
import ProposalResponse from "@/components/ui/ai-interaction/responses/ProposalResponse.vue";
import { createMockSubmit } from "@/components/ui/ai-interaction/submitAdapter";
import type { SubmitResult } from "@/components/ui/ai-interaction/types";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 帧 06/07 — 提案卡。
 *
 * 核心口径：**提案接收与正式审批分离**——
 *   - 接收提案 = 记录评审人选择（accepted），不代表已批准；
 *   - 提交正式审批 = 独立提交（submitted），结果只写「已提交至正式审批流程」。
 * 两条结果互不覆盖，帧 07 把两者并列展示。
 */
const { t } = useI18n();

/** 页面级提交适配器：300ms 延迟用于展示「提交中」 */
const submit = createMockSubmit({ latencyMs: 300 });

/** 帧 07 的两条独立结果（样例数据；真实页面由两次提交分别回填） */
const acceptedResult: SubmitResult = {
  recorded: true,
  reference: "PROP-2041",
  messageKey: "ai.card.submission.recorded"
};

const approvalResult: SubmitResult = {
  recorded: true,
  reference: "APPR-2042",
  messageKey: "ai.proposal.approval.submitted"
};
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.proposal.title"
    subtitle-key="design.aiInteraction.proposal.subtitle"
  >
    <section class="proposal-page__legend">
      <p class="proposal-page__legend-item">
        <span class="proposal-page__legend-label">{{
          t("ai.proposal.accepted.title")
        }}</span>
        <span class="proposal-page__legend-text">
          Accepting records the reviewer's choice and keeps the edited draft
          version; it never changes business data by itself.
        </span>
      </p>
      <p class="proposal-page__legend-item">
        <span class="proposal-page__legend-label">{{
          t("ai.proposal.approval.resultTitle")
        }}</span>
        <span class="proposal-page__legend-text">
          Formal approval is a separate submission with its own outcome.
          Accepting a proposal never starts, replaces or implies it.
        </span>
      </p>
    </section>

    <DesignFrame
      label="06 — Proposal / Review"
      width="wide"
      note="Review: before/after changes, impact, related objects and version. Accept proposal and submit-for-approval are independent actions on the same card."
    >
      <ProposalResponse
        :payload="proposalFixture"
        :submit="submit"
        timestamp="10:20"
      />
    </DesignFrame>

    <DesignFrame
      label="07 — Proposal / Edit & Compare"
      width="wide"
      note="Edit & compare: version + document diff, then the two independent outcomes. Accepting the proposal and submitting it for formal approval are shown side by side and neither overwrites the other."
    >
      <div class="proposal-page__compare">
        <div class="proposal-page__meta">
          <Badge variant="info" size="sm">{{
            proposalFixture.proposalType
          }}</Badge>
          <span class="proposal-page__version">
            {{ t("ai.proposal.version") }} {{ proposalFixture.version }}
          </span>
          <code class="proposal-page__path">{{
            proposalDiffFixture.path
          }}</code>
        </div>

        <ul class="proposal-page__hunks">
          <li
            v-for="(hunk, hunkIndex) in proposalDiffFixture.hunks"
            :key="hunkIndex"
            class="proposal-page__hunk"
          >
            <p class="proposal-page__hunk-header">{{ hunk.header }}</p>
            <ol class="proposal-page__lines">
              <li
                v-for="(line, lineIndex) in hunk.lines"
                :key="lineIndex"
                class="proposal-page__line"
                :class="`is-${line.kind}`"
              >
                <span class="proposal-page__line-mark" aria-hidden="true">{{
                  line.kind === "add" ? "+" : line.kind === "remove" ? "-" : " "
                }}</span>
                <span class="proposal-page__line-text">{{ line.text }}</span>
              </li>
            </ol>
          </li>
        </ul>

        <div class="proposal-page__outcomes">
          <section class="proposal-page__outcome">
            <p class="proposal-page__outcome-label">
              {{ t("ai.proposal.accepted.title") }}
            </p>
            <AiSubmissionStatus phase="success" :result="acceptedResult" />
            <p class="proposal-page__outcome-note">
              {{ t("ai.proposal.accepted.note") }}
            </p>
          </section>

          <section class="proposal-page__outcome">
            <p class="proposal-page__outcome-label">
              {{ t("ai.proposal.approval.resultTitle") }}
            </p>
            <AiSubmissionStatus phase="success" :result="approvalResult" />
          </section>
        </div>
      </div>
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.proposal-page__legend {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.proposal-page__legend-item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
}

.proposal-page__legend-label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
}

.proposal-page__legend-text {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.proposal-page__compare {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.proposal-page__meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.proposal-page__version {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

.proposal-page__path {
  color: var(--ws-text-disabled);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
}

.proposal-page__hunks {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-2);
  list-style: none;
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.proposal-page__hunk-header {
  margin: 0 0 var(--space-1);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.proposal-page__lines {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.proposal-page__line {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-height: var(--size-ai-diff-line);
  padding: 0 var(--space-2);
  border-left: var(--size-indicator) solid transparent;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.proposal-page__line.is-add {
  color: var(--ws-color-success-text);
  background: var(--ws-elevated);
  border-left-color: var(--ws-color-success);
}

.proposal-page__line.is-remove {
  color: var(--ws-color-danger-text);
  background: var(--ws-elevated);
  border-left-color: var(--ws-color-danger);
}

.proposal-page__line.is-context {
  color: var(--ws-text-secondary);
}

.proposal-page__line-mark {
  flex: none;
  width: var(--space-3);
  text-align: center;
}

.proposal-page__line-text {
  min-width: 0;
  word-break: break-word;
}

.proposal-page__outcomes {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
  gap: var(--space-3);
}

.proposal-page__outcome {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.proposal-page__outcome-label {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.proposal-page__outcome :deep(.ai-submission) {
  margin: 0;
}

.proposal-page__outcome-note {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

@media (prefers-reduced-motion: reduce) {
  .proposal-page__line {
    transition: none;
  }
}
</style>
