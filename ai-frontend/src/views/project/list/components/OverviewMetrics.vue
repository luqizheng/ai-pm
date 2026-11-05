<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";

/**
 * 项目概览指标条（Projects List §4）。
 *
 * 四个聚合指标（Total / Recently Active / Requiring Attention / Archived）——
 * 聚合指示而非独立状态或实体；紧凑单行，点击指标应用对应过滤（受控）。
 * 「需要关注」= 存在核验失败（unreachable）仓库的项目，来自真实域模型。
 */
interface Props {
  counts: Record<"total" | "recent" | "attention" | "archived", number>;
  /** 当前高亮键：all / active / attention / archived */
  active: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{ (e: "select", key: string): void }>();
const { t } = useI18n();

const metrics = computed(() => [
  {
    key: "all",
    countKey: "total" as const,
    tone: "info" as StatusIndicatorTone,
    label: t("pm.projects.metricTotal")
  },
  {
    key: "active",
    countKey: "recent" as const,
    tone: "success" as StatusIndicatorTone,
    label: t("pm.projects.metricRecent")
  },
  {
    key: "attention",
    countKey: "attention" as const,
    tone: "warning" as StatusIndicatorTone,
    label: t("pm.projects.metricAttention")
  },
  {
    key: "archived",
    countKey: "archived" as const,
    tone: "neutral" as StatusIndicatorTone,
    label: t("pm.projects.metricArchived")
  }
]);

function isActive(key: string): boolean {
  return props.active === key;
}
</script>

<template>
  <div
    class="proj-summary"
    role="group"
    :aria-label="t('pm.projects.metricsAria')"
  >
    <button
      v-for="metric in metrics"
      :key="metric.key"
      type="button"
      class="proj-summary__item"
      :class="{ 'proj-summary__item--active': isActive(metric.key) }"
      :aria-pressed="isActive(metric.key)"
      @click="emit('select', metric.key)"
    >
      <StatusIndicator :status="metric.tone" size="sm" />
      <span class="proj-summary__count">{{
        counts[metric.countKey] ?? 0
      }}</span>
      <span class="proj-summary__label">{{ metric.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.proj-summary {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.proj-summary__item {
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

.proj-summary__item:hover {
  background: var(--ws-hover-veil);
}

.proj-summary__item--active {
  background: var(--ws-color-accent-surface);
  border-color: var(--accent);
}

.proj-summary__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.proj-summary__count {
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.proj-summary__label {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  white-space: nowrap;
}
</style>
