<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import FormField from "@/components/ui/form-field/FormField.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

/**
 * Free-text feedback field with a character budget and a Cmd/Ctrl+Enter
 * submit shortcut. Reuses FormField + Textarea so the field visuals and the
 * error/description wiring stay identical to the rest of the form system.
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    label?: string;
    placeholder?: string;
    maxLength?: number;
    disabled?: boolean;
    invalid?: boolean;
    errorMessage?: string;
  }>(),
  {
    label: undefined,
    placeholder: undefined,
    maxLength: undefined,
    disabled: false,
    invalid: false,
    errorMessage: undefined
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  submit: [];
}>();

const { t } = useI18n();

const placeholderText = computed(
  () => props.placeholder ?? t("ai.controls.text.placeholder")
);

const count = computed(() => props.modelValue.length);

const hasCounter = computed(() => props.maxLength !== undefined);

const errorText = computed(
  () => props.errorMessage || t("ai.controls.text.invalid")
);

function onInput(value: string) {
  const next =
    props.maxLength !== undefined ? value.slice(0, props.maxLength) : value;
  emit("update:modelValue", next);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Enter" || !(event.ctrlKey || event.metaKey)) return;
  event.preventDefault();
  if (props.disabled) return;
  emit("submit");
}
</script>

<template>
  <div class="ai-text-feedback" @keydown="onKeydown">
    <FormField :label="label" :error="invalid ? errorText : undefined">
      <Textarea
        :model-value="modelValue"
        :placeholder="placeholderText"
        :disabled="disabled"
        :invalid="invalid"
        :rows="3"
        @update:model-value="onInput"
      />
    </FormField>

    <div class="ai-text-feedback__meta">
      <span v-if="hasCounter" class="ai-text-feedback__counter">
        {{ t("ai.controls.text.counter", { count, max: maxLength }) }}
      </span>
      <span class="ai-text-feedback__hint">{{
        t("ai.controls.text.shortcut")
      }}</span>
    </div>
  </div>
</template>

<style scoped>
.ai-text-feedback {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-text-feedback__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  min-height: var(--size-icon-xs);
}

.ai-text-feedback__counter {
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-text-feedback__hint {
  margin-inline-start: auto;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}
</style>
