<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ThumbsDown, ThumbsUp } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";

export type HelpfulValue = "helpful" | "not-helpful" | null;

const props = withDefaults(
  defineProps<{
    modelValue: HelpfulValue;
    disabled?: boolean;
  }>(),
  {
    disabled: false
  }
);

const emit = defineEmits<{ "update:modelValue": [value: HelpfulValue] }>();

const { t } = useI18n();

/** Clicking the active choice again clears it, so the signal stays reversible. */
function pick(value: Exclude<HelpfulValue, null>) {
  if (props.disabled) return;
  emit("update:modelValue", props.modelValue === value ? null : value);
}
</script>

<template>
  <div
    class="ai-helpful"
    role="group"
    :aria-label="t('ai.controls.helpful.group')"
  >
    <Button
      class="ai-helpful__button"
      :class="{ 'is-selected': modelValue === 'helpful' }"
      variant="secondary"
      size="sm"
      :disabled="disabled"
      :aria-pressed="modelValue === 'helpful'"
      @click="pick('helpful')"
    >
      <ThumbsUp :size="14" aria-hidden="true" />
      {{ t("ai.controls.helpful.helpful") }}
    </Button>

    <Button
      class="ai-helpful__button"
      :class="{ 'is-selected': modelValue === 'not-helpful' }"
      variant="secondary"
      size="sm"
      :disabled="disabled"
      :aria-pressed="modelValue === 'not-helpful'"
      @click="pick('not-helpful')"
    >
      <ThumbsDown :size="14" aria-hidden="true" />
      {{ t("ai.controls.helpful.notHelpful") }}
    </Button>
  </div>
</template>

<style scoped>
.ai-helpful {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
}

/* Selected state overrides the shared Button secondary palette on the root. */
.ai-helpful__button.is-selected {
  color: var(--accent);
  background: var(--accent-surface);
  border-color: var(--accent);
}

.ai-helpful__button.is-selected:hover:not(:disabled) {
  color: var(--accent);
  background: var(--accent-surface);
}
</style>
