<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ShieldCheck } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import type { DecisionKind, ReviewProposal } from "../review-contract";

/**
 * 人工决策面板（S02-UI-03 §5）。
 *
 * pending_review：三个决策动作（确认 / 要求修改 / 拒绝）——弹层交互由页根编排；
 * 已决策：展示人类决策记录（评审人 / 时间 / 评论）。
 * 红线呈现：面板明确标注「决策仅能由人类作出；AI 无权确认正式需求」（§1/§6）。
 */
interface Props {
  proposal: ReviewProposal;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "confirm"): void;
  (e: "changes"): void;
  (e: "reject"): void;
}>();

const { t } = useI18n();

const isPending = computed(() => props.proposal.status === "pending_review");

const decisionKindLabel = (kind: DecisionKind) =>
  ({
    confirmed: t("pm.requirementReview.statusConfirmed"),
    changes_requested: t("pm.requirementReview.statusChanges"),
    rejected: t("pm.requirementReview.statusRejected")
  })[kind];

const decisionKindVariant = (
  kind: DecisionKind
): "success" | "info" | "danger" =>
  (
    ({
      confirmed: "success",
      changes_requested: "info",
      rejected: "danger"
    }) as const
  )[kind];
</script>

<template>
  <section
    class="rrp"
    :aria-label="t('pm.requirementReview.decisionPanelTitle')"
  >
    <div class="rrp__head">
      <ShieldCheck class="rrp__shield" aria-hidden="true" />
      <h4 class="rrp__title">
        {{ t("pm.requirementReview.decisionPanelTitle") }}
      </h4>
      <span class="rrp__note">{{
        t("pm.requirementReview.decisionPanelNote")
      }}</span>
    </div>

    <div v-if="isPending" class="rrp__actions">
      <Button variant="primary" size="md" @click="emit('confirm')">
        {{ t("pm.requirementReview.actionConfirm") }}
      </Button>
      <Button variant="secondary" size="md" @click="emit('changes')">
        {{ t("pm.requirementReview.actionChanges") }}
      </Button>
      <Button variant="danger" size="md" @click="emit('reject')">
        {{ t("pm.requirementReview.actionReject") }}
      </Button>
    </div>

    <div v-else-if="props.proposal.decision" class="rrp__record">
      <span class="rrp__record-label">
        {{ t("pm.requirementReview.decisionRecordLabel") }}
      </span>
      <div class="rrp__record-head">
        <Badge
          :variant="decisionKindVariant(props.proposal.decision.kind)"
          size="sm"
        >
          {{ decisionKindLabel(props.proposal.decision.kind) }}
        </Badge>
        <span class="rrp__record-meta">
          {{ props.proposal.decision.reviewer }} ·
          {{ props.proposal.decision.decided_at }}
        </span>
      </div>
      <p v-if="props.proposal.decision.comment" class="rrp__record-comment">
        {{ props.proposal.decision.comment }}
      </p>
    </div>
  </section>
</template>

<style scoped>
.rrp {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-md);
  background: var(--ws-color-accent-surface);
  padding: var(--space-4);
  min-width: 0;
}

.rrp__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rrp__shield {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  color: var(--accent);
}

.rrp__title {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rrp__note {
  margin-left: auto;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rrp__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rrp__record {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rrp__record-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.rrp__record-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rrp__record-meta {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.rrp__record-comment {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border-left: var(--border-2) solid var(--border-default);
  border-radius: 0 var(--radius-xs) var(--radius-xs) 0;
  background: var(--surface-raised);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}
</style>
