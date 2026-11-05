<!--
  StatCard —— 统计小卡（本地延伸组件）。

  ⚠ 非 Figma 同步件：Figma 组件库（hNlGqLEMQpOVCZ0o5u3DkT，41 组）没有统计小卡，
  本组件按 DS 视觉语言本地延伸（同 Badge success/warning 先例）：
  surface 底 + border 1px + radius 8；图标 16px 语义色；数字 32px SemiBold；
  描述 12px 次文字；交互态沿用 hover 薄纱 / focus 2px accent / 骨架占位。
-->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { cn } from "@/lib/cn";

export type StatCardTone =
  | "neutral"
  | "info"
  | "success"
  | "warning"
  | "danger"
  | "accent";
export type StatCardLayout = "stacked" | "compact";
export type StatTrend = { direction: "up" | "down"; value: string };

interface Props {
  /** 指标标签（如 Needs Attention） */
  label: string;
  /** 数值（如 12、95%） */
  value: string;
  /** 数值旁说明（如 Across 3 projects） */
  description?: string;
  /** 头部图标（lucide 组件，随 tone 着色） */
  icon?: Component;
  /** 语义色调（图标 / 趋势 / 点缀） */
  tone?: StatCardTone;
  /** 布局：stacked=上下两行（截图原样）；compact=单行紧凑 */
  layout?: StatCardLayout;
  /** 趋势标记（↑ 12% / ↓ 5%），颜色随方向 */
  trend?: StatTrend;
  /** 右上角操作文案；点击发 action 事件 */
  actionLabel?: string;
  /** 可点选（看板过滤器）：hover 抬升 + focus 环 + selected 态 */
  clickable?: boolean;
  selected?: boolean;
  /** 加载态：数值与描述换骨架条 */
  loading?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  description: undefined,
  icon: undefined,
  tone: "neutral",
  layout: "stacked",
  trend: undefined,
  actionLabel: "",
  clickable: false,
  selected: false,
  loading: false
});

const emit = defineEmits<{ action: []; select: [event: MouseEvent] }>();

const hasTrend = computed(() => Boolean(props.trend));
const hasAction = computed(() => props.actionLabel.length > 0);
</script>

<template>
  <component
    :is="clickable ? 'button' : 'section'"
    :type="clickable ? 'button' : undefined"
    :class="
      cn('ui-stat-card', `ui-stat-card--${layout}`, `is-tone-${tone}`, {
        'is-clickable': clickable,
        'is-selected': selected,
        'is-loading': loading
      })
    "
    :aria-pressed="clickable && selected ? 'true' : undefined"
    :aria-busy="loading ? 'true' : undefined"
    @click="clickable ? emit('select', $event) : undefined"
  >
    <header v-if="icon || actionLabel" class="ui-stat-card__head">
      <span v-if="icon" class="ui-stat-card__label">
        <component :is="icon" class="ui-stat-card__icon" aria-hidden="true" />
        <span class="ui-stat-card__label-text">{{ label }}</span>
      </span>
      <button
        v-if="hasAction"
        type="button"
        class="ui-stat-card__action"
        @click.stop="emit('action')"
      >
        {{ actionLabel }}
      </button>
    </header>

    <!-- compact：label 与数值同排 -->
    <div v-if="layout === 'compact'" class="ui-stat-card__row">
      <span v-if="!icon" class="ui-stat-card__label-text">{{ label }}</span>
      <span class="ui-stat-card__value ui-stat-card__value--compact">{{
        value
      }}</span>
      <span
        v-if="hasTrend"
        :class="cn('ui-stat-card__trend', `is-${trend!.direction}`)"
      >
        {{ trend!.direction === "up" ? "↑" : "↓" }} {{ trend!.value }}
      </span>
    </div>

    <!-- stacked：数字行（截图原样：大数字 + 同排说明） -->
    <div v-else class="ui-stat-card__row">
      <span class="ui-stat-card__value">{{ value }}</span>
      <span v-if="description" class="ui-stat-card__description">{{
        description
      }}</span>
      <span
        v-if="hasTrend"
        :class="cn('ui-stat-card__trend', `is-${trend!.direction}`)"
      >
        {{ trend!.direction === "up" ? "↑" : "↓" }} {{ trend!.value }}
      </span>
    </div>
  </component>
</template>

<style scoped>
.ui-stat-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
  min-width: 0;
  padding: var(--space-3) var(--space-4);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  text-align: left;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    background-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

/* 可点选：button 化 + hover 抬升 + selected accent 面 */
.ui-stat-card.is-clickable {
  cursor: pointer;
}

.ui-stat-card.is-clickable:hover {
  border-color: var(--border-strong);
  box-shadow: var(--shadow-sm);
}

.ui-stat-card.is-clickable:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-stat-card.is-selected {
  background: var(--accent-surface);
  border-color: var(--accent);
}

/* 头行：图标（语义色）+ 标签 13px Medium */
.ui-stat-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.ui-stat-card__label {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.ui-stat-card__label-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.ui-stat-card__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

/* 语义色：图标 / 趋势 */
.is-tone-info .ui-stat-card__icon {
  color: var(--ws-color-info);
}

.is-tone-success .ui-stat-card__icon {
  color: var(--ws-color-success);
}

.is-tone-warning .ui-stat-card__icon {
  color: var(--ws-color-warning);
}

.is-tone-danger .ui-stat-card__icon {
  color: var(--ws-color-danger);
}

.is-tone-accent .ui-stat-card__icon {
  color: var(--accent);
}

/* 数值行：大数字 32 SemiBold + 说明 12 次文字 + 趋势 */
.ui-stat-card__row {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.ui-stat-card__value {
  font-size: var(--font-size-stat);
  line-height: var(--line-height-stat);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.ui-stat-card__value--compact {
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
}

.ui-stat-card__description {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.ui-stat-card__trend {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  font-variant-numeric: tabular-nums;
}

.ui-stat-card__trend.is-up {
  color: var(--ws-color-success-text);
}

.ui-stat-card__trend.is-down {
  color: var(--ws-color-danger-text);
}

/* compact：次文字标签在行首（无图标时） */
.ui-stat-card--compact .ui-stat-card__label-text {
  color: var(--text-secondary);
  font-weight: var(--weight-regular);
}

.ui-stat-card--compact .ui-stat-card__row {
  justify-content: space-between;
}

/* 右上角操作链接 */
.ui-stat-card__action {
  flex: none;
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
  cursor: pointer;
}

.ui-stat-card__action:hover {
  color: var(--accent-hover);
}

.ui-stat-card__action:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--space-1);
  border-radius: var(--radius-xs);
}

/* 加载态：数值与说明换骨架条 */
.ui-stat-card.is-loading .ui-stat-card__value,
.ui-stat-card.is-loading .ui-stat-card__description,
.ui-stat-card.is-loading .ui-stat-card__trend {
  display: inline-block;
  height: var(--line-height-stat);
  width: 72px;
  border-radius: var(--radius-xs);
  background: var(--skeleton-base-strong);
  color: transparent;
  overflow: hidden;
}

.ui-stat-card.is-loading .ui-stat-card__description {
  width: 96px;
  height: var(--line-height-caption);
  background: var(--skeleton-base);
}
</style>
