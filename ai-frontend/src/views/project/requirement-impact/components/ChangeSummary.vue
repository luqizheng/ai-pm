<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Sparkles, UserRound } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import type { ChangeSummaryInfo } from "../impact-contract";

/**
 * 变更摘要卡（S02-UI-07 §3）。
 *
 * 六要素：原始需求 / 拟议变更（对比呈现）/ 变更原因 / 变更来源 /
 * 提议人 / 提交日期。原始与拟议用引用块视觉区分（同来源材料口径）。
 */
interface Props {
  change: ChangeSummaryInfo;
}

const props = defineProps<Props>();
const { t } = useI18n();

const sourceLabel = computed(
  () =>
    ({
      ai_assisted: t("pm.requirementHistory.originAi"),
      human_edit: t("pm.requirementHistory.originHuman"),
      review_meeting: t("pm.requirementImpact.sourceReviewMeeting")
    })[props.change.source] ?? props.change.source
);

const isAiSource = computed(() => props.change.source === "ai_assisted");
</script>

<template>
  <section class="rcs" :aria-label="t('pm.requirementImpact.summaryTitle')">
    <div class="rcs__head">
      <h3 class="rcs__title">{{ t("pm.requirementImpact.summaryTitle") }}</h3>
      <span class="rcs__source" :class="{ 'rcs__source--ai': isAiSource }">
        <component :is="isAiSource ? Sparkles : UserRound" aria-hidden="true" />
        <span>{{ t("pm.requirementImpact.summarySourceLabel") }}</span>
        <span class="rcs__source-value">{{ sourceLabel }}</span>
      </span>
    </div>

    <div class="rcs__compare">
      <div class="rcs__compare-col">
        <span class="rcs__col-label">{{
          t("pm.requirementImpact.summaryOriginal")
        }}</span>
        <blockquote class="rcs__quote">
          {{ props.change.original_summary }}
        </blockquote>
      </div>
      <div class="rcs__compare-col">
        <span class="rcs__col-label">{{
          t("pm.requirementImpact.summaryProposed")
        }}</span>
        <blockquote class="rcs__quote rcs__quote--proposed">
          {{ props.change.proposed_summary }}
        </blockquote>
      </div>
    </div>

    <dl class="rcs__facts">
      <div>
        <dt>{{ t("pm.requirementImpact.summaryReason") }}</dt>
        <dd>{{ props.change.reason }}</dd>
      </div>
      <div>
        <dt>{{ t("pm.requirementImpact.summaryProposedBy") }}</dt>
        <dd>{{ props.change.proposed_by }}</dd>
      </div>
      <div>
        <dt>{{ t("pm.requirementImpact.summarySubmittedAt") }}</dt>
        <dd class="rcs__mono">{{ props.change.submitted_at }}</dd>
      </div>
    </dl>
  </section>
</template>

<style scoped>
.rcs {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rcs__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rcs__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rcs__source {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rcs__source svg {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
}

.rcs__source--ai {
  color: var(--accent);
}

.rcs__source-value {
  color: var(--text-primary);
  font-weight: var(--weight-medium);
}

.rcs__compare {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.rcs__compare-col {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.rcs__col-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.rcs__quote {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border-left: var(--border-2) solid var(--border-default);
  border-radius: 0 var(--radius-xs) var(--radius-xs) 0;
  background: var(--surface-raised);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rcs__quote--proposed {
  border-left-color: var(--accent);
  color: var(--text-primary);
}

.rcs__facts {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.rcs__facts dt {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rcs__facts dd {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rcs__mono {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

@media (max-width: 1100px) {
  .rcs__compare,
  .rcs__facts {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
