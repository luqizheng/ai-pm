<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Forms/TextInput" node=13:32788 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  /** 关联控件 id */
  for?: string;
  /** 必填：显示装饰性星号 */
  required?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  for: undefined,
  required: false
});

const classes = computed(() =>
  cn("ui-label", { "is-required": props.required })
);
</script>

<template>
  <label :for="props.for" :class="classes">
    <slot />
    <span v-if="props.required" class="ui-label__required" aria-hidden="true"
      >*</span
    >
  </label>
</template>

<style scoped>
/* Figma DS/Forms 字段标签：13px Medium（13:32788） */
.ui-label {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}

.ui-label__required {
  color: var(--danger);
}
</style>
