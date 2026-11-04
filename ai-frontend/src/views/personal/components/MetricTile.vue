<script setup lang="ts">
/**
 * My Overview 的单个指标块。
 *
 * 设计约束（见 docs/ui/s01/s01-05-personal-home-high-fidelity-ui-v1.md §4.1）：
 *   - 数字只用于快速概览，不画图表 / 进度条 / 趋势线；
 *   - 颜色点只承载语义（warning=需要关注 / info=分配给我 / accent=活跃项目），
 *     数字本身用主文字色；
 *   - 数据不可用时显示「—」而不是 0（失败 ≠ 空）。
 */
interface Props {
  /** 指标名 */
  label: string;
  /** 指标值；null 表示不可用（加载失败） */
  value: number | null;
  /** 副标题（口径解释） */
  hint?: string;
  /** 语义色点 */
  tone?: "warning" | "info" | "accent" | "success" | "muted";
}

withDefaults(defineProps<Props>(), {
  hint: "",
  tone: "muted"
});

const emit = defineEmits<{ (e: "select"): void }>();
</script>

<template>
  <button type="button" class="pm-metric" @click="emit('select')">
    <span class="pm-metric__dot" :data-tone="tone" aria-hidden="true" />
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
.pm-metric {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4);
  text-align: left;
  border-left: var(--border-1) solid var(--ws-border);
  transition: background var(--duration-fast) var(--ease-standard);
}

.pm-metric:first-child {
  border-left: 0;
}

.pm-metric:hover {
  background: var(--ws-hover-veil);
}

.pm-metric:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.pm-metric__dot {
  width: var(--size-ai-dot);
  height: var(--size-ai-dot);
  margin-top: 7px;
  border-radius: var(--radius-full);
  flex: none;
  background: var(--ws-text-disabled);
}

.pm-metric__dot[data-tone="warning"] {
  background: var(--ws-color-warning);
}

.pm-metric__dot[data-tone="info"] {
  background: var(--ws-color-info);
}

.pm-metric__dot[data-tone="accent"] {
  background: var(--ws-accent);
}

.pm-metric__dot[data-tone="success"] {
  background: var(--ws-color-success);
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
