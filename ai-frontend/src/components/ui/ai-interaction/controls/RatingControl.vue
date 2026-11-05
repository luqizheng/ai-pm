<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Star } from "lucide-vue-next";

/**
 * Star rating with radio-group semantics: one native radio per star keeps
 * arrow-key navigation and screen-reader state native, the stars only paint it.
 */
const props = withDefaults(
  defineProps<{
    modelValue: number;
    max?: number;
    disabled?: boolean;
    labels?: string[];
  }>(),
  {
    max: 5,
    disabled: false,
    labels: () => []
  }
);

const emit = defineEmits<{ "update:modelValue": [value: number] }>();

const { t } = useI18n();

const uid = useId();
const groupName = computed(() => `ai-rating-${uid}`);

const stars = computed(() =>
  Array.from({ length: props.max }, (_, index) => index + 1)
);

function labelFor(star: number) {
  return (
    props.labels[star - 1] ?? t("ai.controls.rating.value", { value: star })
  );
}

function select(star: number) {
  if (props.disabled) return;
  emit("update:modelValue", star);
}
</script>

<template>
  <div
    class="ai-rating"
    role="radiogroup"
    :aria-label="t('ai.controls.rating.group')"
  >
    <label
      v-for="star in stars"
      :key="star"
      class="ai-rating__star"
      :class="{
        'is-active': star <= modelValue,
        'is-disabled': disabled
      }"
    >
      <input
        class="ai-rating__native"
        type="radio"
        :name="groupName"
        :value="star"
        :checked="modelValue === star"
        :disabled="disabled"
        :aria-label="labelFor(star)"
        @change="select(star)"
      />
      <Star
        :size="18"
        :fill="star <= modelValue ? 'currentColor' : 'none'"
        aria-hidden="true"
      />
    </label>
  </div>
</template>

<style scoped>
.ai-rating {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-family: var(--font-ui);
}

.ai-rating__star {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  color: var(--border-strong);
  border-radius: var(--radius-xs);
  cursor: pointer;
  transition: color var(--duration-fast) var(--ease-decelerate);
}

.ai-rating__star:hover:not(.is-disabled) {
  color: var(--warning-strong);
}

.ai-rating__star.is-active {
  color: var(--warning-strong);
}

.ai-rating__star:has(.ai-rating__native:focus-visible) {
  box-shadow: var(--ws-focus-ring);
}

.ai-rating__star.is-disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}

.ai-rating__native {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  margin: 0;
  opacity: 0;
  cursor: inherit;
}

@media (prefers-reduced-motion: reduce) {
  .ai-rating__star {
    transition: none;
  }
}
</style>
