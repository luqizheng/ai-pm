<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { RequirementUiState } from "../requirement-entry";

/**
 * 需求概览摘要条（S02-UI-01 §5）。
 *
 * 五个聚合指标（Total/Draft/Pending/Confirmed/Conflicts）——聚合指示而非
 * 独立工作流或实体；紧凑单行，点击指标按状态过滤列表（受控）。
 */
interface Props {
  counts: Record<"total" | RequirementUiState, number>;
  /** 当前按状态过滤的激活项（空 = 全部） */
  active: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{ (e: "select", key: string): void }>();
const { t } = useI18n();

const metrics = computed(() => [
  {
    key: "all",
    countKey: "total" as const,
    tone: "info" as const,
    label: t("pm.requirements.summaryTotal")
  },
  {
    key: "draft",
    countKey: "draft" as const,
    tone: "neutral" as const,
    label: t("pm.requirements.summaryDraft")
  },
  {
    key: "pending_confirmation",
    countKey: "pending_confirmation" as const,
    tone: "warning" as const,
    label: t("pm.requirements.summaryPending")
  },
  {
    key: "confirmed",
    countKey: "confirmed" as const,
    tone: "success" as const,
    label: t("pm.requirements.summaryConfirmed")
  },
  {
    key: "conflict",
    countKey: "conflict" as const,
    tone: "danger" as const,
    label: t("pm.requirements.summaryConflicts")
  }
]);

function isActive(key: string): boolean {
  return props.active === key;
}
</script>

<template>
  <div
    class="req-summary"
    role="group"
    :aria-label="t('pm.requirements.summaryAria')"
  >
    <button
      v-for="metric in metrics"
      :key="metric.key"
      type="button"
      class="req-summary__item"
      :class="{ 'req-summary__item--active': isActive(metric.key) }"
      :aria-pressed="isActive(metric.key)"
      @click="emit('select', metric.key)"
    >
      <StatusIndicator :status="metric.tone" size="sm" />
      <span class="req-summary__count">{{ counts[metric.countKey] ?? 0 }}</span>
      <span class="req-summary__label">{{ metric.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.req-summary {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.req-summary__item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-sm);
  padding: 0 var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-full);
  background: transparent;
  font-family: var(--font-ui);
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

.req-summary__item:hover {
  background: var(--ws-hover-veil);
}

.req-summary__item--active {
  background: var(--ws-color-accent-surface);
  border-color: var(--accent);
}

.req-summary__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.req-summary__count {
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.req-summary__label {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  white-space: nowrap;
}
</style>
