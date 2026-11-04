<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/Radio" node=13:33076 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

/**
 * Radio —— Figma DS/Forms/Radio（13:33076）。
 * 控件直径 sm14 / md16 / lg20；Checked = accent 底 + 6px canvas 圆点；
 * Focus = 控件 2px accent 描边；Disabled = 整组 0.40。
 * Hover 与 Default 在 Figma 中无视觉差异（仅语义），不单独渲染。
 */

export type RadioSize = "sm" | "md" | "lg";

interface Props {
  label: string;
  /** 绑定值（v-model） */
  modelValue?: string | number | boolean;
  /** 该项的取值（v-model 命中即选中） */
  value?: string | number | boolean;
  size?: RadioSize;
  disabled?: boolean;
  name?: string;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: undefined,
  value: undefined,
  size: "md",
  disabled: false,
  name: undefined
});

const emit = defineEmits<{
  "update:modelValue": [value: string | number | boolean];
}>();

const checked = computed(
  () =>
    props.value !== undefined &&
    props.modelValue !== undefined &&
    props.value === props.modelValue
);

function onSelect() {
  if (!props.disabled && props.value !== undefined) {
    emit("update:modelValue", props.value);
  }
}
</script>

<template>
  <label
    :class="cn('ui-radio', `ui-radio--${size}`, { 'is-disabled': disabled })"
  >
    <input
      type="radio"
      class="ui-radio__native"
      :name="name"
      :checked="checked"
      :disabled="disabled"
      :value="value"
      @change="onSelect"
    />
    <span class="ui-radio__control" aria-hidden="true">
      <span v-if="checked" class="ui-radio__dot" />
    </span>
    <span class="ui-radio__label">{{ label }}</span>
  </label>
</template>

<style scoped>
.ui-radio {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  cursor: pointer;
}

.ui-radio.is-disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}

/* 原生 input 隐藏但保留可达性；焦点态由控件代持 */
.ui-radio__native {
  position: absolute;
  width: 1px;
  height: 1px;
  margin: 0;
  opacity: 0;
}

.ui-radio__control {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-full);
  background-color: var(--ws-canvas);
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate);
}

/* 直径三档 */
.ui-radio--sm .ui-radio__control {
  width: var(--size-radio-sm);
  height: var(--size-radio-sm);
}

.ui-radio--md .ui-radio__control {
  width: var(--size-radio);
  height: var(--size-radio);
}

.ui-radio--lg .ui-radio__control {
  width: var(--size-radio-lg);
  height: var(--size-radio-lg);
}

/* 选中：accent 底 + canvas 圆点 */
.ui-radio__native:checked + .ui-radio__control {
  background-color: var(--accent);
  border-color: var(--border-strong);
}

.ui-radio__dot {
  width: var(--size-radio-dot);
  height: var(--size-radio-dot);
  border-radius: var(--radius-full);
  background-color: var(--ws-canvas);
}

/* 键盘焦点：控件 2px accent 描边 */
.ui-radio__native:focus-visible + .ui-radio__control {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-radio__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}
</style>
