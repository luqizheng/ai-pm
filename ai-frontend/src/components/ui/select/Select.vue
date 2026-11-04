<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/Select" node=13:33018 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import {
  SelectContent,
  SelectIcon,
  SelectItem,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectViewport
} from "reka-ui";
import { useI18n } from "vue-i18n";
import { cn } from "@/lib/cn";
import type { SelectOption } from "./types";

interface Props {
  /** 当前选中值（受控） */
  modelValue?: string;
  /** 选项列表 */
  options?: SelectOption[];
  /** 允许清空：选中项可被清除并回到空值 */
  clearable?: boolean;
  /** 清空按钮的无障碍名称 */
  clearLabel?: string;
  /** 未选中时的占位文案（由调用方注入 i18n） */
  placeholder?: string;
  /** 禁用整个控件 */
  disabled?: boolean;
  /** 错误态：输出 aria-invalid（Phase 2 §6.3） */
  invalid?: boolean;
  /** 表单必填 */
  required?: boolean;
  /** 表单字段名（reka-ui 会生成隐藏 input） */
  name?: string;
  /** 触发器 id，便于 label / aria-labelledby 关联 */
  id?: string;
  /** 尺寸系 — Phase 1R §6.2：sm 32px / md 36px */
  size?: "sm" | "md";
  /** 触发器的无障碍名称 */
  ariaLabel?: string;
  /** 浮层与触发器的对齐方式 */
  align?: "start" | "center" | "end";
  /** 浮层展开方向 */
  side?: "top" | "bottom";
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: undefined,
  options: () => [],
  clearable: false,
  clearLabel: undefined,
  placeholder: "",
  disabled: false,
  invalid: false,
  required: false,
  name: undefined,
  id: undefined,
  size: "md",
  ariaLabel: undefined,
  align: "start",
  side: "bottom"
});

const { t } = useI18n();

// 默认文案走 i18n（调用方显式传入时优先）
const clearLabelText = computed(() => props.clearLabel ?? t("common.clear"));

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const selectedOption = computed(() =>
  props.options.find(option => option.value === props.modelValue)
);

function onClear() {
  emit("update:modelValue", "");
}

function onUpdate(value: unknown) {
  emit("update:modelValue", String(value));
}
</script>

<template>
  <SelectRoot
    :model-value="modelValue"
    :disabled="disabled"
    :required="required"
    :name="name"
    @update:model-value="onUpdate"
  >
    <!-- 触发器：按 Phase 1R §6.2 输入框规格（白底 / border-default / radius-sm / 36px） -->
    <SelectTrigger
      :id="id"
      class="ui-select__trigger"
      :class="cn(`ui-select__trigger--${size}`, { 'is-invalid': invalid })"
      :aria-label="ariaLabel"
      :aria-invalid="invalid ? 'true' : undefined"
    >
      <SelectValue class="ui-select__value" :placeholder="placeholder">
        <template #default="{ selectedLabel }">
          <slot
            name="value"
            :option="selectedOption"
            :selected-label="selectedLabel"
          >
            <template v-if="selectedLabel.length">{{
              selectedLabel.join(", ")
            }}</template>
            <template v-else>{{ placeholder }}</template>
          </slot>
        </template>
      </SelectValue>
      <button
        v-if="clearable && modelValue"
        type="button"
        class="ui-select__clear"
        :aria-label="clearLabelText"
        @click.stop="onClear"
      >
        <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
          <path
            d="M4 4l8 8M12 4l-8 8"
            stroke="currentcolor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
      </button>
      <SelectIcon class="ui-select__icon" aria-hidden="true">
        <slot name="icon">
          <svg
            class="ui-select__chevron"
            viewBox="0 0 16 16"
            focusable="false"
            aria-hidden="true"
          >
            <path
              d="M4 6.5 8 10.5l4-4"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </slot>
      </SelectIcon>
    </SelectTrigger>

    <!-- 浮层：Phase 1R §6.4（白底 + border-strong + shadow-md / radius-md / 内边距 6px / 最小宽 200px） -->
    <SelectPortal>
      <SelectContent
        class="ui-select__content"
        :side="side"
        :align="align"
        :side-offset="4"
      >
        <SelectViewport class="ui-select__viewport">
          <SelectItem
            v-for="option in options"
            :key="option.value"
            :value="option.value"
            :disabled="option.disabled"
            class="ui-select__item"
            :class="{ 'is-selected': option.value === modelValue }"
          >
            <!-- 选中项左侧勾选图标（纯装饰，不参与读屏；SVG 无文本，避免污染选项文案） -->
            <svg
              class="ui-select__check"
              viewBox="0 0 16 16"
              focusable="false"
              aria-hidden="true"
            >
              <path
                d="M3.5 8.5l3 3 6-6.5"
                fill="none"
                stroke="currentColor"
                stroke-width="1.6"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
            <SelectItemText class="ui-select__item-text">
              <slot name="item" :option="option">{{ option.label }}</slot>
            </SelectItemText>
          </SelectItem>
        </SelectViewport>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>

<style scoped>
/* Figma DS/Forms/Select（13:33018）：触发器同 TextInput 口径（canvas 底 /
 * border 描边 / radius 8 / sm28 md36 lg44）；下拉面板 elevated 底 + radius 8 +
 * 内边距 4；选项行 h36 / radius 8 / 主文字，选中与悬停底 = accent 着色面。 */
.ui-select__trigger {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  width: 100%;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
  background: var(--surface-sunken);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}

.ui-select__trigger:hover:not([data-disabled]) {
  border-color: var(--border-hover-strong);
}

/* 聚焦 —— Figma：accent 2px 描边（inset 阴影补 1px 免布局抖动） */
.ui-select__trigger:focus-visible {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 var(--border-1) var(--accent);
  outline: none;
}

.ui-select__trigger[data-disabled] {
  background: var(--surface-raised);
  opacity: var(--opacity-disabled-field);
  cursor: not-allowed;
}

/* 错误态 —— Figma：#E69A9F 描边 */
.ui-select__trigger.is-invalid {
  border-color: var(--danger-title);
}

.ui-select__trigger.is-invalid:focus-visible {
  box-shadow: inset 0 0 0 var(--border-1) var(--danger-title);
}

/* 尺寸系 —— Figma 三档 */
.ui-select__trigger--sm {
  height: var(--size-control-sm);
}

.ui-select__trigger--lg {
  height: var(--size-control-lg);
}

.ui-select__value {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 占位文案：次文字色 */
.ui-select__value[data-placeholder] {
  color: var(--text-secondary);
}

.ui-select__clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.ui-select__clear:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-select__clear:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-select__icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
}

.ui-select__chevron {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

/* 浮层容器 — Figma Options 面板：elevated 底 / border 描边 / radius 8 / 内边距 4 */
:deep(.ui-select__content) {
  min-width: var(--size-menu-min-width);
  padding: var(--space-1);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  animation: ui-select-enter var(--duration-fast) var(--ease-decelerate);
}

:deep(.ui-select__viewport) {
  padding: 0;
}

/* 菜单项 — Figma：h36 / radius 8 / padding 0 8 / 主文字 13 Regular */
.ui-select__item {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-md);
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
  cursor: pointer;
  user-select: none;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

/* 悬停/键盘高亮 —— Figma：accent 着色面底 */
.ui-select__item[data-highlighted] {
  background: var(--accent-surface);
}

.ui-select__item[data-disabled] {
  opacity: var(--opacity-disabled-option, 0.35);
  cursor: not-allowed;
}

/* 选中项 —— accent 着色面底 + 主文字 + 勾号 */
.ui-select__item.is-selected {
  background: var(--accent-surface);
}

.ui-select__check {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--accent);
  opacity: 0;
}

.ui-select__item.is-selected .ui-select__check {
  opacity: 1;
}

.ui-select__item-text {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@keyframes ui-select-enter {
  from {
    opacity: 0;
    transform: scale(0.96);
  }

  to {
    opacity: 1;
    transform: scale(1);
  }
}

/* 减少动效 — 仅 opacity 80ms，无 scale */
@media (prefers-reduced-motion: reduce) {
  :deep(.ui-select__content) {
    animation: ui-select-enter-instant var(--duration-instant) linear;
  }

  @keyframes ui-select-enter-instant {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }
}
</style>
