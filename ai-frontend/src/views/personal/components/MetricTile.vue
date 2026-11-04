<script setup lang="ts">
import { computed } from "vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";

/**
 * My Overview 的单个指标块（页内私有布局件）。
 *
 * 设计约束（见 docs/ui/s01/s01-05-personal-home-high-fidelity-ui-v1.md §4.1）：
 *   - 数字只用于快速概览，不画图表 / 进度条 / 趋势线；
 *   - 语义点用公共 StatusIndicator，不自造色点；
 *   - 数据不可用时显示「—」而不是 0（失败 ≠ 空）。
 */
interface Props {
  /** 指标名 */
  label: string;
  /** 指标值；null 表示不可用（加载失败） */
  value: number | null;
  /** 副标题（口径解释） */
  hint?: string;
  /** 语义档（映射到 StatusIndicator 的 status） */
  tone?: "warning" | "info" | "accent" | "success" | "muted";
}

const props = withDefaults(defineProps<Props>(), {
  hint: "",
  tone: "muted"
});

const emit = defineEmits<{ (e: "select"): void }>();

const status = computed(() => {
  const byTone = {
    warning: "warning",
    info: "info",
    accent: "accent",
    success: "success",
    muted: "neutral"
  } as const;
  return byTone[props.tone];
});
</script>

<template>
  <button type="button" class="pm-metric" @click="emit('select')">
    <StatusIndicator :status="status" size="sm" />
    <span class="pm-metric__body">
      <span class="pm-metric__value" :class="{ 'is-na': value === null }">
        {{ value === null ? "—" : value }}
      </span>
      <span class="pm-metric__label">{{ label }}</span>
      <span v-if="hint" class="pm-metric__hint">{{ hint }}</span>
    </span>
  </button>
</template>

<style scoped>
/* 裸 <button> 必须显式清掉浏览器默认 buttonface 底色（违和根因） */
.pm-metric {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4);
  text-align: left;
  background: transparent;
  border: 0;
  border-left: var(--border-1) solid var(--ws-border);
  font: inherit;
  color: inherit;
  cursor: pointer;
  transition: background-color var(--duration-fast) var(--ease-standard);
}

.pm-metric:first-child {
  border-left: 0;
}

.pm-metric:hover {
  background-color: var(--ws-hover-veil);
}

.pm-metric:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.pm-metric__body {
  min-width: 0;
}

.pm-metric__value {
  display: block;
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  letter-spacing: -0.01em;
  font-variant-numeric: tabular-nums;
}

.pm-metric__value.is-na {
  color: var(--ws-text-disabled);
}

.pm-metric__label {
  display: block;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-primary);
  font-weight: var(--weight-medium);
}

.pm-metric__hint {
  display: block;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}
</style>
