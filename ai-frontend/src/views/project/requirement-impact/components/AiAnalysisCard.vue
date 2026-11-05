<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Sparkles } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import type { ImpactAiAnalysis } from "../impact-contract";

/**
 * AI 分析卡（S02-UI-07 §6）。
 *
 * 影响摘要 / 风险评估 / 证据 / 置信度 / 建议。
 * 红线：整卡标注「AI 生成分析，非确认事实」——AI 结论不作为已确认事实呈现。
 */
interface Props {
  ai: ImpactAiAnalysis;
}

const props = defineProps<Props>();
const { t } = useI18n();

const riskVariant = computed(
  () =>
    ({
      low: "success",
      medium: "warning",
      high: "danger"
    })[props.ai.risk] as "success" | "warning" | "danger"
);

const confidenceVariant = computed(
  () =>
    ({
      high: "success",
      medium: "info",
      low: "neutral"
    })[props.ai.confidence] as "success" | "info" | "neutral"
);

const riskLabel = computed(
  () =>
    ({
      low: t("pm.requirementImpact.riskLow"),
      medium: t("pm.requirementImpact.riskMedium"),
      high: t("pm.requirementImpact.riskHigh")
    })[props.ai.risk]
);

const confidenceLabel = computed(
  () =>
    ({
      high: t("pm.requirementImpact.confidenceHigh"),
      medium: t("pm.requirementImpact.confidenceMedium"),
      low: t("pm.requirementImpact.confidenceLow")
    })[props.ai.confidence]
);
</script>

<template>
  <section class="ria" :aria-label="t('pm.requirementImpact.aiTitle')">
    <div class="ria__head">
      <h3 class="ria__title">
        <Sparkles class="ria__spark" aria-hidden="true" />
        {{ t("pm.requirementImpact.aiTitle") }}
      </h3>
      <Badge variant="accent" size="sm" outline>
        {{ t("pm.requirementImpact.aiBadge") }}
      </Badge>
    </div>

    <p class="ria__disclaimer">{{ t("pm.requirementImpact.aiDisclaimer") }}</p>

    <p class="ria__summary">{{ props.ai.summary }}</p>

    <div class="ria__metrics">
      <span class="ria__metric">
        <span class="ria__metric-label">{{
          t("pm.requirementImpact.aiRisk")
        }}</span>
        <Badge :variant="riskVariant" size="sm">{{ riskLabel }}</Badge>
      </span>
      <span class="ria__metric">
        <span class="ria__metric-label">{{
          t("pm.requirementImpact.aiConfidence")
        }}</span>
        <Badge :variant="confidenceVariant" size="sm" outline>
          {{ confidenceLabel }}
        </Badge>
      </span>
    </div>

    <div class="ria__section">
      <span class="ria__section-label">{{
        t("pm.requirementImpact.aiEvidence")
      }}</span>
      <ol class="ria__list">
        <li v-for="(item, index) in props.ai.evidence" :key="`ev-${index}`">
          {{ item }}
        </li>
      </ol>
    </div>

    <div class="ria__section">
      <span class="ria__section-label">{{
        t("pm.requirementImpact.aiRecommendations")
      }}</span>
      <ol class="ria__list">
        <li
          v-for="(item, index) in props.ai.recommendations"
          :key="`rec-${index}`"
        >
          {{ item }}
        </li>
      </ol>
    </div>
  </section>
</template>

<style scoped>
.ria {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-md);
  background: var(--ws-color-accent-surface);
  padding: var(--space-4);
  min-width: 0;
}

.ria__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.ria__title {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.ria__spark {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--accent);
}

.ria__disclaimer {
  margin: 0;
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.ria__summary {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.ria__metrics {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  flex-wrap: wrap;
}

.ria__metric {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.ria__metric-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.ria__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ria__section-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.ria__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding-left: var(--space-5);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-primary);
}

.ria__list li::marker {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
</style>
