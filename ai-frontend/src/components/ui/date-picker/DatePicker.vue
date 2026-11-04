<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/DatePicker" node=13:33274 date=2026-11-04 -->
<script setup lang="ts">
import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  CircleAlert
} from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * DatePicker —— Figma DS/Forms/DatePicker（13:33274）。
 * Label + 输入行（calendar 图标 + 值 + error 警示图标）+ 帮助文案 +
 * Open 态日历面板（月份导航 / 周头 M T W T F S S / 6×7 日期格 / Today·Clear 页脚）。
 * 日历数据经 props 注入（演示期默认 2026-11 样例）；选中日 accent 底 + canvas 文字。
 */

export type DatePickerSize = "sm" | "md" | "lg";

interface Props {
  label?: string;
  modelValue?: string;
  helpText?: string;
  /** 错误提示（覆盖 helpText 并切换 error 态） */
  error?: string;
  open?: boolean;
  size?: DatePickerSize;
  disabled?: boolean;
  /** 日历月份标题（如 November 2026） */
  monthLabel?: string;
  /** 6×7 日期格文字；空串 = 非本月占位 */
  weeks?: string[][];
  /** 选中日（1-based，在 weeks 中的文字） */
  selectedDay?: string;
}

const props = withDefaults(defineProps<Props>(), {
  label: "Due date",
  modelValue: "Choose a date",
  helpText: "Date format: MMM DD, YYYY",
  error: undefined,
  open: false,
  size: "md",
  disabled: false,
  monthLabel: "November 2026",
  weeks: () => [
    ["26", "27", "28", "29", "30", "31", "1"],
    ["2", "3", "4", "5", "6", "7", "8"],
    ["9", "10", "11", "12", "13", "14", "15"],
    ["16", "17", "18", "19", "20", "21", "22"],
    ["23", "24", "25", "26", "27", "28", "29"],
    ["30", "1", "2", "3", "4", "5", "6"]
  ],
  selectedDay: "18"
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
  "update:open": [value: boolean];
  select: [day: string];
  today: [];
  clear: [];
}>();

const WEEKDAYS = ["M", "T", "W", "T", "F", "S", "S"];

function onToggle() {
  if (!props.disabled) emit("update:open", !props.open);
}

function onPickDay(day: string) {
  if (!day) return;
  emit("select", day);
  emit("update:modelValue", day);
  emit("update:open", false);
}

const hasError = () => Boolean(props.error);
</script>

<template>
  <div
    :class="
      cn('ui-datepicker', `ui-datepicker--${size}`, {
        'is-disabled': disabled,
        'is-error': hasError(),
        'is-open': open
      })
    "
  >
    <p class="ui-datepicker__label">{{ label }}</p>

    <button
      type="button"
      class="ui-datepicker__field"
      :disabled="disabled"
      :aria-expanded="open ? 'true' : 'false'"
      @click="onToggle"
    >
      <CalendarDays class="ui-datepicker__icon" aria-hidden="true" />
      <span class="ui-datepicker__value">{{ modelValue }}</span>
      <CircleAlert
        v-if="hasError()"
        class="ui-datepicker__alert"
        aria-hidden="true"
      />
    </button>

    <p :class="cn('ui-datepicker__help', { 'is-error': hasError() })">
      {{ error ?? helpText }}
    </p>

    <div v-if="open" class="ui-datepicker__calendar">
      <div class="ui-datepicker__month">
        <button
          type="button"
          class="ui-datepicker__month-nav"
          aria-label="Previous month"
          @click.stop
        >
          <ChevronLeft aria-hidden="true" />
        </button>
        <span class="ui-datepicker__month-label">{{ monthLabel }}</span>
        <button
          type="button"
          class="ui-datepicker__month-nav"
          aria-label="Next month"
          @click.stop
        >
          <ChevronRight aria-hidden="true" />
        </button>
      </div>

      <div class="ui-datepicker__grid" role="grid">
        <div class="ui-datepicker__week ui-datepicker__week--head">
          <span
            v-for="(d, i) in WEEKDAYS"
            :key="`wd-${i}`"
            class="ui-datepicker__day ui-datepicker__day--head"
            aria-hidden="true"
          >
            {{ d }}
          </span>
        </div>
        <div
          v-for="(week, wi) in weeks"
          :key="`wk-${wi}`"
          class="ui-datepicker__week"
        >
          <button
            v-for="(day, di) in week"
            :key="'d-' + wi + '-' + di"
            type="button"
            :class="
              cn('ui-datepicker__day', {
                'is-out': !day.length,
                'is-selected': day.length > 0 && day === selectedDay
              })
            "
            @click="onPickDay(day)"
          >
            {{ day }}
          </button>
        </div>
      </div>

      <div class="ui-datepicker__footer">
        <button
          type="button"
          class="ui-datepicker__action ui-datepicker__action--today"
          @click.stop="emit('today')"
        >
          Today
        </button>
        <button
          type="button"
          class="ui-datepicker__action"
          @click.stop="emit('clear')"
        >
          Clear
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ui-datepicker {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
}

.ui-datepicker.is-disabled {
  opacity: var(--opacity-disabled-field);
}

.ui-datepicker__label {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.ui-datepicker__field {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  background-color: var(--ws-canvas);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    background-color var(--duration-fast) var(--ease-decelerate);
}

.ui-datepicker--sm .ui-datepicker__field {
  height: var(--size-control-sm);
}

.ui-datepicker--md .ui-datepicker__field {
  height: var(--size-control-md);
}

.ui-datepicker--lg .ui-datepicker__field {
  height: var(--size-control-lg);
}

.ui-datepicker__field:hover:not(:disabled):not(.is-error) {
  background-color: var(--surface-raised);
}

.ui-datepicker__field:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

/* Open 态：1px accent 描边（区别 Focus 的 2px） */
.ui-datepicker.is-open .ui-datepicker__field {
  border-color: var(--accent);
}

/* Error 态：1px danger 描边 + 警示图标 */
.ui-datepicker.is-error .ui-datepicker__field {
  border-color: var(--danger-title);
}

.ui-datepicker__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

.ui-datepicker__value {
  flex: 1;
  min-width: 0;
  text-align: left;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
}

.ui-datepicker__alert {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--danger-title);
}

.ui-datepicker__help {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.ui-datepicker__help.is-error {
  color: var(--danger-title);
}

/* 日历面板：elevated 底 + border 描边 + radius 8 */
.ui-datepicker__calendar {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background-color: var(--surface-raised);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
}

.ui-datepicker__month {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.ui-datepicker__month-nav {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-button);
  height: var(--size-icon-button);
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.ui-datepicker__month-nav:hover {
  background-color: var(--ws-hover-veil);
}

.ui-datepicker__month-nav svg {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

.ui-datepicker__month-label {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.ui-datepicker__grid {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ui-datepicker__week {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: var(--space-1);
}

.ui-datepicker__day {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: var(--size-calendar-day);
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  cursor: pointer;
}

.ui-datepicker__day--head {
  color: var(--text-secondary);
  cursor: default;
}

.ui-datepicker__day.is-out {
  color: var(--text-disabled);
}

.ui-datepicker__day.is-selected {
  background-color: var(--accent);
  color: var(--accent-contrast-text);
}

.ui-datepicker__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.ui-datepicker__action {
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  cursor: pointer;
}

.ui-datepicker__action--today {
  color: var(--accent);
}

.ui-datepicker__action:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--space-1);
  border-radius: var(--radius-xs);
}
</style>
