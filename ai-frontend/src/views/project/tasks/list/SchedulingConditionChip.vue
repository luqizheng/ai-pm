<script setup lang="ts">
import type { SchedulingCondition } from "../tasks-contract";

/**
 * Scheduling Condition 徽标（PA-07 179:3603「Dependency Condition Indicator」）。
 *
 * 与任务生命周期状态徽标（TaskStatusChip）并行展示、绝不替代——组件说明原文：
 * 「Task lifecycle only. Never use dependency blocking or Product suspension as
 * a Task lifecycle state」。scheduled 紫 = --ws-color-status-scheduled。
 */
const props = defineProps<{ condition: SchedulingCondition }>();
</script>

<template>
  <span
    class="sc-chip"
    :class="`sc-chip--${props.condition.tone}`"
    :data-condition="props.condition.key"
  >
    <span class="sc-chip__label">{{ props.condition.label }}</span>
    <span v-if="props.condition.detail" class="sc-chip__detail">{{
      props.condition.detail
    }}</span>
  </span>
</template>

<style scoped>
.sc-chip {
  display: inline-flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-1);
}

.sc-chip__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: 500;
  line-height: var(--line-height-micro);
}

.sc-chip__detail {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

.sc-chip--neutral .sc-chip__label {
  color: var(--ws-text-primary);
}

.sc-chip--info .sc-chip__label {
  color: var(--ws-color-info-text);
}

.sc-chip--danger .sc-chip__label {
  color: var(--ws-color-danger-text);
}

.sc-chip--scheduled .sc-chip__label {
  color: var(--ws-color-status-scheduled);
}

.sc-chip--success .sc-chip__label {
  color: var(--ws-color-success-text);
}

.sc-chip--muted .sc-chip__label {
  color: var(--ws-text-disabled);
}

.sc-chip--warning .sc-chip__label {
  color: var(--ws-color-warning-text);
}

.sc-chip--ready .sc-chip__label {
  color: var(--ws-color-status-ready);
}
</style>
