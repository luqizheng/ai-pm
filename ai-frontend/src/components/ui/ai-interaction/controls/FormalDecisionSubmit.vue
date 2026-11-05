<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import Button from "@/components/ui/button/Button.vue";
import Checkbox from "@/components/ui/checkbox/Checkbox.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

/**
 * Formal decision submission block.
 *
 * Submitting a decision is a human act: when `requireConfirm` is set the user
 * must tick "I confirm submitting as myself" before the button unlocks.
 */
const props = withDefaults(
  defineProps<{
    selectedOption?: string;
    comment?: string;
    submitting?: boolean;
    disabled?: boolean;
    requireConfirm?: boolean;
  }>(),
  {
    selectedOption: undefined,
    comment: "",
    submitting: false,
    disabled: false,
    requireConfirm: false
  }
);

const emit = defineEmits<{
  "update:comment": [value: string];
  submit: [];
}>();

const { t } = useI18n();

const confirmed = ref(false);

const hasSelection = computed(() => {
  if (props.selectedOption === undefined) return true;
  return props.selectedOption.length > 0;
});

const canSubmit = computed(
  () =>
    !props.disabled &&
    !props.submitting &&
    hasSelection.value &&
    (!props.requireConfirm || confirmed.value)
);

function submit() {
  if (!canSubmit.value) return;
  emit("submit");
}
</script>

<template>
  <div class="ai-formal-decision">
    <p v-if="selectedOption" class="ai-formal-decision__selected">
      <span class="ai-formal-decision__selected-label">{{
        t("ai.controls.formalDecision.selected")
      }}</span>
      <span class="ai-formal-decision__selected-value">{{
        selectedOption
      }}</span>
    </p>

    <FormField
      :label="t('ai.controls.formalDecision.commentLabel')"
      :description="t('ai.controls.formalDecision.commentPlaceholder')"
    >
      <Textarea
        :model-value="comment"
        :placeholder="t('ai.controls.formalDecision.commentPlaceholder')"
        :disabled="disabled || submitting"
        :rows="2"
        @update:model-value="emit('update:comment', $event)"
      />
    </FormField>

    <div class="ai-formal-decision__footer">
      <Checkbox
        v-if="requireConfirm"
        :model-value="confirmed"
        :label="t('ai.controls.formalDecision.confirm')"
        :disabled="disabled || submitting"
        @update:model-value="confirmed = $event === true"
      />

      <Button
        class="ai-formal-decision__submit"
        variant="primary"
        size="sm"
        :loading="submitting"
        :disabled="!canSubmit"
        @click="submit"
      >
        {{ t("ai.controls.formalDecision.submit") }}
      </Button>
    </div>
  </div>
</template>

<style scoped>
.ai-formal-decision {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-formal-decision__selected {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-2) var(--space-3);
  background: var(--accent-surface);
  border-radius: var(--radius-xs);
}

.ai-formal-decision__selected-label {
  color: var(--accent);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-formal-decision__selected-value {
  color: var(--text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.ai-formal-decision__footer {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.ai-formal-decision__submit {
  margin-inline-start: auto;
}
</style>
