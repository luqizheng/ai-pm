<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import { informationalFixture } from "@/components/ui/ai-interaction/fixtures";
import InformationalResponse from "@/components/ui/ai-interaction/responses/InformationalResponse.vue";
import type { InformationalPayload } from "@/components/ui/ai-interaction/types";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 帧 02 — 信息卡。
 *
 * 信息卡是只读的：卡片只上报 copy / ask-follow-up 两个信息性动作，
 * 页面侧给出可见确认；不出现接收 / 拒绝 / 审批类按钮。
 */
const { t } = useI18n();

/** 最近上报的动作（页面级可见反馈） */
const lastAction = ref("");
const followUpVisible = ref(false);

function onAction(key: string) {
  lastAction.value = key;
  if (key === "ask-follow-up") followUpVisible.value = true;
}

/** 无来源样例：只保留正文首句，来源段按需缺省 */
const noSourcePayload: InformationalPayload = {
  contentMarkdown: informationalFixture.contentMarkdown.split("\n")[0]
};
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.informational.title"
    subtitle-key="design.aiInteraction.informational.subtitle"
  >
    <DesignFrame
      label="02 — Informational / Analysis & sources"
      width="wide"
      note="Read-only card: copy and follow-up are the only actions. Copy is acknowledged inside the card; follow-up is surfaced here as a page-level acknowledgement."
    >
      <InformationalResponse
        :payload="informationalFixture"
        timestamp="10:01"
        @action="onAction"
      />

      <p
        v-if="followUpVisible"
        class="info-page__feedback"
        role="status"
        aria-live="polite"
      >
        <span class="info-page__feedback-label">{{
          t("ai.informational.followUp")
        }}</span>
        <code class="info-page__feedback-action">{{ lastAction }}</code>
        <span class="info-page__feedback-note">emitted</span>
      </p>
    </DesignFrame>

    <DesignFrame
      label="02a — Informational / Without sources"
      width="narrow"
      note="Evidence is optional: when payload.sources is empty the sources block is skipped entirely and the card keeps its compact height."
    >
      <InformationalResponse
        :payload="noSourcePayload"
        density="compact"
        timestamp="10:06"
      />
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.info-page__feedback {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: var(--space-3) 0 0;
  padding: var(--space-2) var(--space-3);
  color: var(--ws-text-primary);
  background: var(--ws-elevated);
  border-left: var(--size-indicator) solid var(--ws-accent);
  border-radius: var(--radius-xs);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.info-page__feedback-label {
  font-weight: var(--weight-medium);
}

.info-page__feedback-action {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.info-page__feedback-note {
  color: var(--ws-text-disabled);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
}
</style>
