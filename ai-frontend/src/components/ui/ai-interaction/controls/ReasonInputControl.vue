<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import FormField from "@/components/ui/form-field/FormField.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

/**
 * Reason capture for reject / change-request flows.
 *
 * When `required` is set the empty value is reported as invalid immediately
 * (no touch tracking) because the caller only renders it after the user
 * explicitly opened the reject path.
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    label?: string;
    placeholder?: string;
    required?: boolean;
    disabled?: boolean;
    invalid?: boolean;
    errorMessage?: string;
    autoFocus?: boolean;
  }>(),
  {
    label: undefined,
    placeholder: undefined,
    required: false,
    disabled: false,
    invalid: false,
    errorMessage: undefined,
    autoFocus: false
  }
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const { t } = useI18n();

const rootRef = ref<HTMLElement | null>(null);

const placeholderText = computed(
  () => props.placeholder ?? t("ai.controls.reason.placeholder")
);

const empty = computed(() => props.modelValue.trim().length === 0);

const showError = computed(
  () => props.invalid || (props.required && empty.value)
);

const errorText = computed(
  () => props.errorMessage || t("ai.controls.reason.required")
);

onMounted(() => {
  if (!props.autoFocus) return;
  rootRef.value?.querySelector("textarea")?.focus();
});
</script>

<template>
  <div ref="rootRef" class="ai-reason-input">
    <FormField
      :label="label ?? t('ai.controls.reason.label')"
      :required="required"
      :error="showError ? errorText : undefined"
    >
      <Textarea
        :model-value="modelValue"
        :placeholder="placeholderText"
        :disabled="disabled"
        :invalid="showError"
        :rows="3"
        @update:model-value="emit('update:modelValue', $event)"
      />
    </FormField>
  </div>
</template>

<style scoped>
.ai-reason-input {
  display: block;
  min-width: 0;
  font-family: var(--font-ui);
}
</style>
