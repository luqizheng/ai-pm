<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { CircleAlert } from "lucide-vue-next";

import type { QuestionOption } from "../types";

/**
 * Single choice rendered as description rows.
 *
 * Native radio inputs keep the WAI-ARIA radio-group pattern (arrow keys, roving
 * focus) for free; the visible row is the label of its own input.
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    options: QuestionOption[];
    name?: string;
    disabled?: boolean;
    required?: boolean;
    invalid?: boolean;
    errorMessage?: string;
    label?: string;
  }>(),
  {
    name: undefined,
    disabled: false,
    required: false,
    invalid: false,
    errorMessage: undefined,
    label: undefined
  }
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const { t } = useI18n();

const uid = useId();
const labelId = computed(() => `ai-single-select-label-${uid}`);
const errorId = computed(() => `ai-single-select-error-${uid}`);
const groupName = computed(() => props.name ?? `ai-single-select-${uid}`);

/** Only surface the required error after the first interaction (no flash on mount). */
const touched = ref(false);

const showError = computed(
  () => props.invalid || (props.required && touched.value && !props.modelValue)
);

const errorText = computed(
  () => props.errorMessage || t("ai.controls.select.required")
);

function select(value: string) {
  if (props.disabled) return;
  touched.value = true;
  emit("update:modelValue", value);
}
</script>

<template>
  <div class="ai-control">
    <p v-if="label" :id="labelId" class="ai-control__label">{{ label }}</p>

    <div
      class="ai-single-select"
      role="radiogroup"
      :aria-labelledby="label ? labelId : undefined"
      :aria-label="label ? undefined : t('ai.controls.select.groupLabel')"
      :aria-invalid="showError ? 'true' : undefined"
      :aria-describedby="showError ? errorId : undefined"
    >
      <label
        v-for="option in options"
        :key="option.value"
        class="ai-single-select__option"
        :class="{
          'is-selected': option.value === modelValue,
          'is-disabled': disabled
        }"
      >
        <input
          class="ai-single-select__native"
          type="radio"
          :name="groupName"
          :value="option.value"
          :checked="option.value === modelValue"
          :disabled="disabled"
          @change="select(option.value)"
        />
        <span class="ai-single-select__marker" aria-hidden="true" />
        <span class="ai-single-select__body">
          <span class="ai-single-select__option-label">{{ option.label }}</span>
          <span
            v-if="option.description"
            class="ai-single-select__description"
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

.ai-single-select {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.ai-single-select__option {
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

.ai-single-select__option:hover:not(.is-disabled) {
  background: var(--surface-raised);
  border-color: var(--border-hover-strong);
}

.ai-single-select__option.is-selected {
  background: var(--accent-surface);
  border-color: var(--accent);
}

/* Keyboard focus follows the native input, the row renders the ring. */
.ai-single-select__option:has(.ai-single-select__native:focus-visible) {
  box-shadow: var(--ws-focus-ring);
}

.ai-single-select__option.is-disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}

.ai-single-select__native {
  position: absolute;
  top: var(--space-2);
  inset-inline-start: var(--space-3);
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  margin: 0;
  opacity: 0;
}

.ai-single-select__marker {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  margin-top: var(--space-1);
  background: transparent;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-full);
}

.ai-single-select__option.is-selected .ai-single-select__marker {
  border-color: var(--accent);
}

.ai-single-select__option.is-selected .ai-single-select__marker::after {
  content: "";
  width: var(--space-2);
  height: var(--space-2);
  background: var(--accent);
  border-radius: var(--radius-full);
}

.ai-single-select__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.ai-single-select__option-label {
  color: var(--text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.ai-single-select__description {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

@media (prefers-reduced-motion: reduce) {
  .ai-single-select__option {
    transition: none;
  }
}
</style>
