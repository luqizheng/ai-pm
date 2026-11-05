<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ShieldAlert, Sparkles, UserRound, Inbox } from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type {
  ProposalSource,
  ReviewFilterKey,
  ReviewProposal,
  ReviewStatus
} from "../review-contract";

/**
 * 评审队列（S02-UI-03 §3，左栏 35%）。
 *
 * 五档过滤器（All / Pending / Changes Requested / Confirmed / Rejected）+
 * 待评审计数；行显示提案编号、标题、类型、提案来源、提交时间、评审状态与
 * 冲突指示器。提案来源徽标区分 AI / 人类 / 导入（§1/§6 AI 与人类可辨）。
 */
interface Props {
  proposals: ReviewProposal[];
  selectedId: string;
  filter: ReviewFilterKey;
  /** 各档计数（页根 computed 传入） */
  counts: Record<ReviewFilterKey | "all", number>;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "select", id: string): void;
  (e: "update:filter", value: ReviewFilterKey): void;
}>();

const { t } = useI18n();

const filters = computed(() => [
  { key: "all" as const, label: t("pm.requirementReview.filterAll") },
  {
    key: "pending_review" as const,
    label: t("pm.requirementReview.filterPending"),
    badge: props.counts.pending_review
  },
  {
    key: "changes_requested" as const,
    label: t("pm.requirementReview.filterChanges")
  },
  {
    key: "confirmed" as const,
    label: t("pm.requirementReview.filterConfirmed")
  },
  { key: "rejected" as const, label: t("pm.requirementReview.filterRejected") }
]);

const sourceMeta: Record<ProposalSource, { icon: Component; key: string }> = {
  ai: { icon: Sparkles, key: "sourceAi" },
  human: { icon: UserRound, key: "sourceHuman" },
  import: { icon: Inbox, key: "sourceImport" }
};

const statusMeta: Record<
  ReviewStatus,
  { key: string; variant: "warning" | "info" | "success" | "danger" }
> = {
  pending_review: { key: "statusPending", variant: "warning" },
  changes_requested: { key: "statusChanges", variant: "info" },
  confirmed: { key: "statusConfirmed", variant: "success" },
  rejected: { key: "statusRejected", variant: "danger" }
};

const typeLabel = (type: string) =>
  type === "functional"
    ? t("pm.requirements.typeFunctional")
    : t("pm.requirements.typeNonFunctional");

const sourceLabel = (source: ProposalSource) =>
  t(`pm.requirementReview.${sourceMeta[source].key}`);
</script>

<template>
  <section class="rrq" :aria-label="t('pm.requirementReview.queueAria')">
    <div class="rrq__head">
      <h3 class="rrq__title">{{ t("pm.requirementReview.queueTitle") }}</h3>
      <Badge variant="warning" size="sm" outline>
        {{
          t("pm.requirementReview.pendingCount", {
            count: props.counts.pending_review
          })
        }}
      </Badge>
    </div>

    <div
      class="rrq__filters"
      role="group"
      :aria-label="t('pm.requirementReview.filterAria')"
    >
      <button
        v-for="item in filters"
        :key="item.key"
        type="button"
        class="rrq__filter"
        :class="{ 'rrq__filter--active': props.filter === item.key }"
        :aria-pressed="props.filter === item.key"
        @click="emit('update:filter', item.key)"
      >
        {{ item.label }}
      </button>
    </div>

    <EmptyState
      v-if="props.proposals.length === 0"
      size="sm"
      :title="t('pm.requirementReview.queueEmptyTitle')"
      :description="t('pm.requirementReview.queueEmptyDesc')"
    />
    <ul v-else class="rrq__list">
      <li v-for="proposal in props.proposals" :key="proposal.id">
        <button
          type="button"
          class="rrq__row"
          :class="{ 'rrq__row--selected': proposal.id === props.selectedId }"
          :aria-current="proposal.id === props.selectedId ? 'true' : undefined"
          @click="emit('select', proposal.id)"
        >
          <span class="rrq__row-top">
            <span class="rrq__code">{{ proposal.code }}</span>
            <span
              v-if="proposal.has_conflict"
              class="rrq__conflict"
              :title="t('pm.requirementReview.conflictIndicator')"
            >
              <ShieldAlert aria-hidden="true" />
            </span>
            <Badge :variant="statusMeta[proposal.status].variant" size="sm">
              {{ t(`pm.requirementReview.${statusMeta[proposal.status].key}`) }}
            </Badge>
          </span>
          <span class="rrq__title">{{ proposal.title }}</span>
          <span class="rrq__row-meta">
            <Badge variant="neutral" size="sm">{{
              typeLabel(proposal.type)
            }}</Badge>
            <span
              class="rrq__source"
              :class="`rrq__source--${proposal.source}`"
            >
              <component
                :is="sourceMeta[proposal.source].icon"
                aria-hidden="true"
              />
              <span>{{ sourceLabel(proposal.source) }}</span>
            </span>
            <span class="rrq__time">{{ proposal.submitted_at }}</span>
          </span>
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.rrq {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rrq__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.rrq__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rrq__filters {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  flex-wrap: wrap;
}

.rrq__filter {
  padding: var(--space-1) var(--space-2);
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-full);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard);
}

.rrq__filter:hover {
  background: var(--ws-hover-veil);
  color: var(--text-primary);
}

.rrq__filter--active {
  background: var(--ws-color-accent-surface);
  border-color: var(--accent);
  color: var(--accent);
}

.rrq__filter:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.rrq__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
  overflow-y: auto;
}

.rrq__row {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: transparent;
  padding: var(--space-3);
  font-family: var(--font-ui);
  text-align: left;
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

.rrq__row:hover {
  background: var(--ws-hover-veil);
}

.rrq__row--selected {
  background: var(--ws-color-accent-surface);
  border-color: var(--accent);
}

.rrq__row:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.rrq__row-top {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.rrq__code {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rrq__conflict {
  display: inline-flex;
  align-items: center;
  color: var(--danger-strong);
}

.rrq__conflict svg {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

.rrq__title {
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.rrq__row-meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rrq__source {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rrq__source svg {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
}

.rrq__source--ai {
  color: var(--accent);
}

.rrq__time {
  margin-left: auto;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
</style>
