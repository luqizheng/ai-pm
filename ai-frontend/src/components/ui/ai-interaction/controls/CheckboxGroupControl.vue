<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import { CircleAlert } from "lucide-vue-next";

import Checkbox from "@/components/ui/checkbox/Checkbox.vue";
import type { QuestionOption } from "../types";

const props = withDefaults(
  defineProps<{
    modelValue: string[];
    options: QuestionOption[];
    orientation?: "vertical" | "horizontal";
    disabled?: boolean;
    invalid?: boolean;
    errorMessage?: string;
    label?: string;
  }>(),
  {
    orientation: "vertical",
    disabled: false,
    invalid: false,
    errorMessage: undefined,
    label: undefined
  }
);

const emit = defineEmits<{ "update:modelValue": [value: string[]] }>();

const { t } = useI18n();

const uid = useId();
const labelId = computed(() => `ai-checkbox-group-label-${uid}`);
const errorId = computed(() => `ai-checkbox-group-error-${uid}`);

const errorText = computed(
  () => props.errorMessage || t("ai.controls.checkboxGroup.required")
);

function toggle(value: string, checked: boolean | "indeterminate") {
  if (props.disabled || checked === "indeterminate") return;
  const current = props.modelValue;
  const next = checked
    ? current.includes(value)
      ? current
      : [...current, value]
    : current.filter(item => item !== value);
  emit("update:modelValue", next);
}
</script>

<template>
  <div class="ai-control">
    <p v-if="label" :id="labelId" class="ai-control__label">{{ label }}</p>

    <div
      class="ai-checkbox-group"
      :class="`is-${orientation}`"
      role="group"
      :aria-labelledby="label ? labelId : undefined"
      :aria-invalid="invalid ? 'true' : undefined"
      :aria-describedby="invalid ? errorId : undefined"
    >
      <div
        v-for="option in options"
        :key="option.value"
        class="ai-checkbox-group__option"
      >
        <Checkbox
          :model-value="modelValue.includes(option.value)"
          :label="option.label"
          :disabled="disabled"
          @update:model-value="toggle(option.value, $event)"
        />
        <span
          v-if="option.description"
          class="ai-checkbox-group__description"
          >{{ option.description }}</span
        >
      </div>
    </div>

    <p v-if="invalid" :id="errorId" class="ai-control__error" role="alert">
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

.ai-checkbox-group {
  display: flex;
  gap: var(--space-2);
  min-width: 0;
}

.ai-checkbox-group.is-vertical {
  flex-direction: column;
}

.ai-checkbox-group.is-horizontal {
  flex-direction: row;
  flex-wrap: wrap;
}

.ai-checkbox-group__option {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.ai-checkbox-group__description {
  padding-inline-start: var(--space-5);
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}
</style>
