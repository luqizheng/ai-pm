<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";

import Select from "@/components/ui/select/Select.vue";
import type { SelectOption } from "@/components/ui/select/types";
import type { QuestionOption } from "../types";

/**
 * Thin wrapper over the shared `Select` so question cards can render a
 * dropdown with the same option contract as the other choice controls.
 */
const props = withDefaults(
  defineProps<{
    modelValue?: string;
    options: QuestionOption[];
    label?: string;
    placeholder?: string;
    invalid?: boolean;
    helpText?: string;
    disabled?: boolean;
  }>(),
  {
    modelValue: "",
    label: undefined,
    placeholder: undefined,
    invalid: false,
    helpText: undefined,
    disabled: false
  }
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const { t } = useI18n();

const uid = useId();
const selectId = computed(() => `ai-select-dropdown-${uid}`);

const placeholderText = computed(
  () => props.placeholder ?? t("ai.controls.select.placeholder")
);

const selectOptions = computed<SelectOption[]>(() =>
  props.options.map(option => ({ value: option.value, label: option.label }))
);

function onUpdate(value: string) {
  emit("update:modelValue", value);
}
</script>

<template>
  <div class="ai-control" :class="{ 'is-invalid': invalid }">
    <label v-if="label" :for="selectId" class="ai-control__label">{{
      label
    }}</label>

    <Select
      :id="selectId"
      :model-value="modelValue"
      :options="selectOptions"
      :placeholder="placeholderText"
      :disabled="disabled"
      :invalid="invalid"
      :aria-label="label ? undefined : t('ai.controls.select.groupLabel')"
      @update:model-value="onUpdate"
    />

    <p v-if="helpText" class="ai-control__help">{{ helpText }}</p>
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

.ai-control__help {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-control.is-invalid .ai-control__help {
  color: var(--danger-title);
}
</style>
