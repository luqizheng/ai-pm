<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/Switch" node=13:33107 date=2026-11-04 -->
<script setup lang="ts">
import { computed, useId } from "vue";
import { SwitchRoot, SwitchThumb } from "reka-ui";

interface Props {
  /** 开关状态（受控） */
  modelValue?: boolean;
  /** 禁用整个开关 */
  disabled?: boolean;
  /** 表单必填 */
  required?: boolean;
  /** 表单字段名（reka-ui 在 form 内生成隐藏 input） */
  name?: string;
  /** 触发器 id，便于 label / aria-labelledby 关联 */
  id?: string;
  /** 尺寸档 —— Figma：sm/md 32×20（thumb 14）；lg 40×24（thumb 18） */
  size?: "sm" | "md" | "lg";
  /** 可见标签文案（由调用方注入 i18n） */
  label?: string;
  /** 标签是否仅对读屏可见 */
  hideLabel?: boolean;
  /** 无障碍名称（未提供 label 时使用，例如纯图标场景） */
  ariaLabel?: string;
  /**
   * 读屏专用状态文案（由调用方注入 i18n）。
   * 组件自身不硬编码中文。
   */
  stateLabel?: { on: string; off: string };
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: false,
  disabled: false,
  required: false,
  name: undefined,
  id: undefined,
  size: "md",
  label: undefined,
  hideLabel: false,
  ariaLabel: undefined,
  stateLabel: undefined
});

const emit = defineEmits<{ "update:modelValue": [value: boolean] }>();

const uid = useId();
/** 根按钮 id：未显式传入时自动生成，保证 label 与 aria-describedby 可关联 */
const rootId = computed(() => props.id ?? `ui-switch-${uid}`);
const labelId = computed(() => `ui-switch-label-${uid}`);
const stateId = computed(() => `ui-switch-state-${uid}`);

/** 当前状态对应的读屏文案 */
const stateText = computed(() =>
  props.modelValue ? props.stateLabel?.on : props.stateLabel?.off
);

function onUpdate(value: unknown) {
  emit("update:modelValue", Boolean(value));
}
</script>

<template>
  <div class="ui-switch">
    <!-- 根节点即 role=switch 的按钮：reka-ui 负责 ARIA 语义与状态 -->
    <SwitchRoot
      :id="rootId"
      :class="['ui-switch__root', `ui-switch__root--${size}`]"
      :model-value="modelValue"
      :disabled="disabled"
      :required="required"
      :name="name"
      :aria-label="label ? undefined : ariaLabel"
      :aria-labelledby="label ? labelId : undefined"
      :aria-describedby="stateText ? stateId : undefined"
      @update:model-value="onUpdate"
    >
      <SwitchThumb class="ui-switch__thumb" />
    </SwitchRoot>

    <!-- 状态播报：仅读屏可见 -->
    <span v-if="stateText" :id="stateId" class="ui-sr-only">{{
      stateText
    }}</span>

    <!-- 可见标签：与根按钮通过 for/id 关联，点击标签即切换 -->
    <label
      v-if="label"
      :id="labelId"
      class="ui-switch__label"
      :class="{ 'ui-sr-only': hideLabel }"
      :for="rootId"
    >
      {{ label }}
    </label>
  </div>
</template>

<style scoped>
/* Figma DS/Forms/Switch（13:33107）：
 * Off 轨道 elevated + border 描边、thumb 次文字色；
 * On 轨道 accent、thumb canvas 色；focus = accent 2px 描边；
 * hover 无样式变化；禁用 = 整体（含标签）0.4 不透明度。 */
.ui-switch {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

/* 整体禁用（含标签）—— Figma Disabled 态口径 */
.ui-switch:has(.ui-switch__root[data-disabled]) {
  opacity: var(--opacity-disabled-control);
}

/* 轨道 —— sm/md 32×20 / lg 40×24，内边距 3px（Figma 逐值） */
.ui-switch__root {
  position: relative;
  display: inline-flex;
  align-items: center;
  box-sizing: border-box;
  flex: none;
  width: var(--size-switch-track);
  height: var(--size-switch-thumb);
  padding: 3px;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-full);
  background: var(--surface-raised);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard),
    box-shadow var(--duration-fast) var(--ease-standard);
}

.ui-switch__root--lg {
  width: var(--size-switch-track-lg);
  height: var(--size-switch-thumb-lg);
}

/* 开启态：底 accent（描边保持 border 色） */
.ui-switch__root[data-state="checked"] {
  background: var(--accent);
}

/* 焦点 —— Figma：accent 2px 描边（inset 阴影补 1px 免布局抖动） */
.ui-switch__root:focus-visible {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 var(--border-1) var(--accent);
  outline: none;
}

.ui-switch__root[data-disabled] {
  cursor: not-allowed;
}

/* 滑块 —— sm/md 14 / lg 18，随轨道内边距 3px 定位 */
.ui-switch__thumb {
  position: absolute;
  top: 50%;
  left: 3px;
  width: var(--size-switch-thumb);
  height: var(--size-switch-thumb);
  border-radius: var(--radius-full);
  background: var(--text-secondary);
  transform: translateY(-50%);
  transition: transform var(--duration-fast) var(--ease-standard);
}

.ui-switch__root--lg .ui-switch__thumb {
  width: var(--size-switch-thumb-lg);
  height: var(--size-switch-thumb-lg);
}

/* 开启后右移至轨道右端（行程 = 轨道宽 - 2×3px - thumb） */
.ui-switch__root[data-state="checked"] .ui-switch__thumb {
  transform: translateY(-50%)
    translateX(
      calc(
        var(--size-switch-track) - var(--size-switch-thumb) - 6px -
          var(--border-1) * 2
      )
    );
}

.ui-switch__root--lg[data-state="checked"] .ui-switch__thumb {
  transform: translateY(-50%)
    translateX(
      calc(
        var(--size-switch-track-lg) - var(--size-switch-thumb-lg) - 6px -
          var(--border-1) * 2
      )
    );
}

/* 开启态滑块 —— canvas 色 */
.ui-switch__root[data-state="checked"] .ui-switch__thumb {
  background: var(--accent-contrast-text);
}

.ui-switch__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
  cursor: pointer;
}

/* 读屏专用文本 —— 不依赖尚未引入的全局 base.css */
.ui-sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
  border: 0;
}

/* 减少动效 —— 位移瞬时完成 */
@media (prefers-reduced-motion: reduce) {
  .ui-switch__thumb {
    transition-duration: var(--duration-instant);
  }
}
</style>
