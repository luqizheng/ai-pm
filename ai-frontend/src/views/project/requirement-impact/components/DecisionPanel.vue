<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ShieldCheck, UserRound } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import type {
  ChangeImpactAnalysis,
  ImpactDecisionKind
} from "../impact-contract";

/**
 * 人类决策面板（S02-UI-07 §7 + Deliverable 4）。
 *
 * 接受前复述变更摘要与影响发现计数（§7 Before acceptance...）；
 * 三动作：接受拟议变更 / 要求更多分析 / 拒绝变更——弹层编排由页根承担，
 * 接受与拒绝必须经显式确认弹窗（Human confirmation is mandatory）。
 * 红线：决策仅能由人类作出；AI 分析不构成已确认事实。
 */
interface Props {
  analysis: ChangeImpactAnalysis;
  decision: {
    kind: ImpactDecisionKind;
    actor: string;
    acted_at: string;
  } | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "accept"): void;
  (e: "more-analysis"): void;
  (e: "reject"): void;
}>();

const { t } = useI18n();

const directCount = computed(
  () =>
    props.analysis.targets.filter(target => target.level === "direct").length
);
const findingCount = computed(() => props.analysis.findings.length);

const decisionLabel = (kind: ImpactDecisionKind) =>
  ({
    accepted: t("pm.requirementImpact.decisionAccepted"),
    more_analysis: t("pm.requirementImpact.decisionMoreAnalysis"),
    rejected: t("pm.requirementImpact.decisionRejected")
  })[kind];

const decisionVariant = (kind: ImpactDecisionKind) =>
  ({
    accepted: "success",
    more_analysis: "info",
    rejected: "danger"
  })[kind] as "success" | "info" | "danger";

const hasDecision = computed(() => props.decision !== null);
</script>

<template>
  <section class="rid" :aria-label="t('pm.requirementImpact.decisionTitle')">
    <div class="rid__head">
      <ShieldCheck class="rid__shield" aria-hidden="true" />
      <h3 class="rid__title">{{ t("pm.requirementImpact.decisionTitle") }}</h3>
      <UserRound class="rid__human-icon" aria-hidden="true" />
      <span class="rid__note">{{
        t("pm.requirementImpact.decisionNote")
      }}</span>
    </div>

    <div v-if="!hasDecision" class="rid__body">
      <div class="rid__recap">
        <p class="rid__recap-item">
          <span class="rid__recap-label">{{
            t("pm.requirementImpact.summaryProposed")
          }}</span>
          {{ props.analysis.change.proposed_summary }}
        </p>
        <p class="rid__recap-item">
          <span class="rid__recap-label">{{
            t("pm.requirementImpact.decisionRecapFindings")
          }}</span>
          {{
            t("pm.requirementImpact.decisionRecapText", {
              findings: findingCount,
              direct: directCount
            })
          }}
        </p>
      </div>

      <div class="rid__actions">
        <Button variant="primary" size="md" @click="emit('accept')">
          {{ t("pm.requirementImpact.decisionAccept") }}
        </Button>
        <Button variant="secondary" size="md" @click="emit('more-analysis')">
          {{ t("pm.requirementImpact.decisionMoreAnalysis") }}
        </Button>
        <Button variant="danger" size="md" @click="emit('reject')">
          {{ t("pm.requirementImpact.decisionReject") }}
        </Button>
      </div>
    </div>

    <div v-else-if="props.decision" class="rid__record">
      <Badge :variant="decisionVariant(props.decision.kind)" size="sm">
        {{ decisionLabel(props.decision.kind) }}
      </Badge>
      <span class="rid__record-meta">
        {{ props.decision.actor }} · {{ props.decision.acted_at }}
      </span>
    </div>
  </section>
</template>

<style scoped>
.rid {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-md);
  background: var(--ws-color-accent-surface);
  padding: var(--space-4);
  min-width: 0;
}

.rid__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rid__shield {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  color: var(--accent);
}

.rid__title {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rid__human-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-secondary);
  margin-left: auto;
}

.rid__note {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rid__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rid__recap {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rid__recap-item {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border-left: var(--border-2) solid var(--border-default);
  border-radius: 0 var(--radius-xs) var(--radius-xs) 0;
  background: var(--surface-raised);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rid__recap-label {
  display: block;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  margin-bottom: var(--space-1);
}

.rid__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rid__record {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rid__record-meta {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
</style>
