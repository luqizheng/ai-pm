<script setup lang="ts">
import { ref } from "vue";

import { actionRequestFixture } from "@/components/ui/ai-interaction/fixtures";
import ActionRequestResponse from "@/components/ui/ai-interaction/responses/ActionRequestResponse.vue";
import { createMockSubmit } from "@/components/ui/ai-interaction/submitAdapter";
import type { ActionRequestPayload } from "@/components/ui/ai-interaction/types";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 交付帧 14 —— Action Request 响应卡。
 *
 * 14 高后果操作：Confirm 先弹 ConfirmationDialog，确认后才提交；
 * 14b 低风险变体：确认直接提交、不弹层；取消只关闭请求，不产生任何业务变更。
 * 红线：提交成功只写「已提交执行」，实际完成状态以执行记录为准。
 */
const submit = createMockSubmit({ latencyMs: 300 });
const lowRiskSubmit = createMockSubmit({ latencyMs: 300 });

/** 同一动作的低风险变体：用于展示「不弹二次确认 / 可取消」的一侧 */
const lowRiskPayload: ActionRequestPayload = {
  ...actionRequestFixture,
  riskLevel: "low",
  consequential: false
};

const cancelNotice = ref("");

function onLowRiskAction(key: string): void {
  if (key === "cancel") {
    cancelNotice.value =
      "Cancel pressed: the request stays pending, nothing is submitted and no business data changes. Cancel is not a rejection.";
  }
}
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.action.title"
    subtitle-key="design.aiInteraction.action.subtitle"
  >
    <section class="action-page__legend">
      <p class="action-page__legend-item">
        <span class="action-page__legend-label">Consequential</span>
        <span class="action-page__legend-text">
          riskLevel medium/high + consequential: Confirm opens the
          ConfirmationDialog first; the request is submitted only from the
          dialog. No success state exists before the adapter resolves.
        </span>
      </p>
      <p class="action-page__legend-item">
        <span class="action-page__legend-label">Low risk / cancel</span>
        <span class="action-page__legend-text">
          riskLevel low with consequential = false submits straight from the
          footer. Cancel only dismisses the request — it is neither a rejection
          nor a business change.
        </span>
      </p>
    </section>

    <DesignFrame
      label="14 — Action Request / Consequential (confirmation dialog)"
      width="default"
      note="Live card: press Confirm to open the confirmation dialog (Teleport → body), then confirm the submit. The success note says the action was submitted for execution — the execution record remains authoritative."
    >
      <ActionRequestResponse
        :payload="actionRequestFixture"
        :submit="submit"
        timestamp="10:30"
      />
    </DesignFrame>

    <DesignFrame
      label="14b — Action Request / Low risk &amp; cancel"
      width="default"
      note="Low-risk variant of the same payload (riskLevel = low, consequential = false). Confirm submits without a dialog; pressing Cancel keeps the request pending and records nothing — the page echoes the cancel outcome below the card."
    >
      <ActionRequestResponse
        :payload="lowRiskPayload"
        :submit="lowRiskSubmit"
        timestamp="10:32"
        @action="onLowRiskAction"
      />
      <p
        v-if="cancelNotice"
        class="action-page__notice"
        role="status"
        aria-live="polite"
      >
        {{ cancelNotice }}
      </p>
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.action-page__legend {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.action-page__legend-item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
}

.action-page__legend-label {
  flex: none;
  min-width: var(--space-12);
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.action-page__legend-text {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.action-page__notice {
  margin: var(--space-3) 0 0;
  padding: var(--space-2) var(--space-3);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  background: var(--ws-elevated);
  border-left: var(--size-indicator) solid var(--ws-border);
  border-radius: var(--radius-xs);
}
</style>
