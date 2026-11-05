<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Check, CircleAlert } from "lucide-vue-next";

import type { QuestionOption } from "../types";

/**
 * Multi choice with min / max selection bounds.
 *
 * The bound is enforced twice: unselected options become unavailable at `max`,
 * and `min` surfaces as a validation error once the field was touched.
 */
const props = withDefaults(
  defineProps<{
    modelValue: string[];
    options: QuestionOption[];
    min?: number;
    max?: number;
    disabled?: boolean;
    invalid?: boolean;
    errorMessage?: string;
    label?: string;
  }>(),
  {
    min: undefined,
    max: undefined,
    disabled: false,
    invalid: false,
    errorMessage: undefined,
    label: undefined
  }
);

const emit = defineEmits<{ "update:modelValue": [value: string[]] }>();

const { t } = useI18n();

const uid = useId();
const labelId = computed(() => `ai-multi-select-label-${uid}`);
const errorId = computed(() => `ai-multi-select-error-${uid}`);

const touched = ref(false);

const count = computed(() => props.modelValue.length);

const atMax = computed(
  () => props.max !== undefined && count.value >= props.max
);

const brokenMin = computed(
  () => props.min !== undefined && count.value < props.min
);

const overMax = computed(
  () => props.max !== undefined && count.value > props.max
);

const showError = computed(
  () => props.invalid || (touched.value && (brokenMin.value || overMax.value))
);

const errorText = computed(() => {
  if (props.errorMessage) return props.errorMessage;
  if (overMax.value && props.max !== undefined) {
    return t("ai.controls.multi.max", { count: props.max });
  }
  if (brokenMin.value && props.min !== undefined) {
    return t("ai.controls.multi.min", { count: props.min });
  }
  return t("ai.controls.multi.invalid");
});

function isSelected(value: string) {
  return props.modelValue.includes(value);
}

function isOptionDisabled(value: string) {
  if (props.disabled) return true;
  return atMax.value && !isSelected(value);
}

function toggle(value: string) {
  if (isOptionDisabled(value)) return;
  touched.value = true;
  const next = isSelected(value)
    ? props.modelValue.filter(item => item !== value)
    : [...props.modelValue, value];
  emit("update:modelValue", next);
}
</script>

<template>
  <div class="ai-control">
    <p v-if="label" :id="labelId" class="ai-control__label">{{ label }}</p>

    <div
      class="ai-multi-select"
      role="group"
      :aria-labelledby="label ? labelId : undefined"
      :aria-invalid="showError ? 'true' : undefined"
      :aria-describedby="showError ? errorId : undefined"
    >
      <label
        v-for="option in options"
        :key="option.value"
        class="ai-multi-select__option"
        :class="{
          'is-selected': isSelected(option.value),
          'is-disabled': isOptionDisabled(option.value)
        }"
      >
        <input
          class="ai-multi-select__native"
          type="checkbox"
          :value="option.value"
          :checked="isSelected(option.value)"
          :disabled="isOptionDisabled(option.value)"
          @change="toggle(option.value)"
        />
        <span class="ai-multi-select__marker" aria-hidden="true">
          <Check v-if="isSelected(option.value)" :size="12" />
        </span>
        <span class="ai-multi-select__body">
          <span class="ai-multi-select__option-label">{{ option.label }}</span>
          <span
            v-if="option.description"
            class="ai-multi-select__description"
            >{{ option.description }}</span
          >
        </span>
      </label>
    </div>

    <p v-if="showError" :id="errorId" class="ai-control__error" role="alert">
      <CircleAlert :size="12" aria-hidden="true" />
      <span>{{ errorText }}</span>
    </p>
  </div>
</template>

<style scoped>
.ai-control {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-control__label {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
}

.ai-control__error {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
  color: var(--danger-title);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-multi-select {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.ai-multi-select__option {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  min-height: var(--size-ai-option-min-height);
  padding: var(--space-2) var(--space-3);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    background-color var(--duration-fast) var(--ease-decelerate);
}

.ai-multi-select__option:hover:not(.is-disabled) {
  background: var(--surface-raised);
  border-color: var(--border-hover-strong);
}

.ai-multi-select__option.is-selected {
  background: var(--accent-surface);
  border-color: var(--accent);
}

.ai-multi-select__option:has(.ai-multi-select__native:focus-visible) {
  box-shadow: var(--ws-focus-ring);
}

.ai-multi-select__option.is-disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}

.ai-multi-select__native {
  position: absolute;
  top: var(--space-2);
  inset-inline-start: var(--space-3);
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  margin: 0;
  opacity: 0;
}

.ai-multi-select__marker {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  margin-top: var(--space-1);
  color: var(--accent-contrast-text);
  background: transparent;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-xs);
}

.ai-multi-select__option.is-selected .ai-multi-select__marker {
  background: var(--accent);
  border-color: var(--accent);
}

.ai-multi-select__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.ai-multi-select__option-label {
  color: var(--text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.ai-multi-select__description {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

@media (prefers-reduced-motion: reduce) {
  .ai-multi-select__option {
    transition: none;
  }
}
</style>
