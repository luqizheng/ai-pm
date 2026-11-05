<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Check, Sparkles, X } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import type { DecisionOption } from "../types";

/**
 * Decision options as selectable cards: recommended badge, summary, pros/cons.
 * Single-select semantics come from the native radio hidden inside each card.
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    options: DecisionOption[];
    disabled?: boolean;
  }>(),
  {
    disabled: false
  }
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const { t } = useI18n();

const uid = useId();
const groupName = computed(() => `ai-decision-option-${uid}`);

function select(id: string) {
  if (props.disabled) return;
  emit("update:modelValue", id);
}
</script>

<template>
  <div
    class="ai-decision-option-selector"
    role="radiogroup"
    :aria-label="t('ai.controls.decision.group')"
  >
    <label
      v-for="option in options"
      :key="option.id"
      class="ai-decision-option-selector__option"
      :class="{
        'is-selected': option.id === modelValue,
        'is-disabled': disabled
      }"
    >
      <span class="ai-decision-option-selector__head">
        <input
          class="ai-decision-option-selector__native"
          type="radio"
          :name="groupName"
          :value="option.id"
          :checked="option.id === modelValue"
          :disabled="disabled"
          @change="select(option.id)"
        />
        <span class="ai-decision-option-selector__marker" aria-hidden="true" />
        <span class="ai-decision-option-selector__label">{{
          option.label
        }}</span>
        <Badge
          v-if="option.recommended"
          class="ai-decision-option-selector__badge"
          variant="accent"
          size="sm"
        >
          <Sparkles :size="12" aria-hidden="true" />
          {{ t("ai.controls.decision.recommended") }}
        </Badge>
      </span>

      <span
        v-if="option.summary"
        class="ai-decision-option-selector__summary"
        >{{ option.summary }}</span
      >

      <span
        v-if="option.pros?.length"
        class="ai-decision-option-selector__list"
      >
        <span class="ai-decision-option-selector__list-title">{{
          t("ai.controls.decision.pros")
        }}</span>
        <span
          v-for="pro in option.pros"
          :key="pro"
          class="ai-decision-option-selector__item"
        >
          <Check
            class="ai-decision-option-selector__icon is-pro"
            :size="12"
            aria-hidden="true"
          />
          {{ pro }}
        </span>
      </span>

      <span
        v-if="option.cons?.length"
        class="ai-decision-option-selector__list"
      >
        <span class="ai-decision-option-selector__list-title">{{
          t("ai.controls.decision.cons")
        }}</span>
        <span
          v-for="con in option.cons"
          :key="con"
          class="ai-decision-option-selector__item"
        >
          <X
            class="ai-decision-option-selector__icon is-con"
            :size="12"
            aria-hidden="true"
          />
          {{ con }}
        </span>
      </span>
    </label>
  </div>
</template>

<style scoped>
.ai-decision-option-selector {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  font-family: var(--font-ui);
}

.ai-decision-option-selector__option {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    background-color var(--duration-fast) var(--ease-decelerate);
}

.ai-decision-option-selector__option:hover:not(.is-disabled) {
  background: var(--surface-raised);
  border-color: var(--border-hover-strong);
}

.ai-decision-option-selector__option.is-selected {
  background: var(--accent-surface);
  border-color: var(--accent);
}

.ai-decision-option-selector__option:has(
  .ai-decision-option-selector__native:focus-visible
) {
  box-shadow: var(--ws-focus-ring);
}

.ai-decision-option-selector__option.is-disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}

.ai-decision-option-selector__native {
  position: absolute;
  top: var(--space-3);
  inset-inline-start: var(--space-3);
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  margin: 0;
  opacity: 0;
}

.ai-decision-option-selector__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.ai-decision-option-selector__marker {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  background: transparent;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-full);
}

.ai-decision-option-selector__option.is-selected
  .ai-decision-option-selector__marker {
  border-color: var(--accent);
}

.ai-decision-option-selector__option.is-selected
  .ai-decision-option-selector__marker::after {
  content: "";
  width: var(--space-2);
  height: var(--space-2);
  background: var(--accent);
  border-radius: var(--radius-full);
}

.ai-decision-option-selector__label {
  color: var(--text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  line-height: var(--line-height-body);
}

.ai-decision-option-selector__badge {
  flex: none;
}

.ai-decision-option-selector__summary {
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-decision-option-selector__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding-inline-start: var(--space-6);
}

.ai-decision-option-selector__list-title {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  line-height: var(--line-height-micro);
  letter-spacing: var(--tracking-caption);
}

.ai-decision-option-selector__item {
  display: inline-flex;
  align-items: flex-start;
  gap: var(--space-1);
  color: var(--text-primary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.ai-decision-option-selector__icon {
  flex: none;
  margin-top: var(--space-1);
}

.ai-decision-option-selector__icon.is-pro {
  color: var(--success-strong);
}

.ai-decision-option-selector__icon.is-con {
  color: var(--danger-strong);
}

@media (prefers-reduced-motion: reduce) {
  .ai-decision-option-selector__option {
    transition: none;
  }
}
</style>
