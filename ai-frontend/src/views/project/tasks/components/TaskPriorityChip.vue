<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { priorityTone } from "../tasks-contract";

/** 任务优先级芯片：urgent/high/normal/low → 语义色调（同状态芯片口径） */
const props = defineProps<{ priority: string }>();

const { t } = useI18n();

const tone = computed(() => priorityTone(props.priority));
const label = computed(() => t(`s04.tasks.priority.${props.priority}`));
</script>

<template>
  <span
    class="task-chip"
    :class="`task-chip--${tone}`"
    :data-priority="props.priority"
  >
    {{ label }}
  </span>
</template>

<style scoped>
.task-chip {
  display: inline-flex;
  align-items: center;
  padding: 0 var(--space-2);
  height: var(--size-control-sm);
  border-radius: var(--radius-full, var(--radius-sm));
  border: var(--border-1) solid transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  white-space: nowrap;
}

.task-chip--neutral {
  background: var(--ws-elevated);
  color: var(--ws-text-secondary);
}

.task-chip--info {
  color: var(--ws-color-info);
  border-color: var(--ws-color-info);
}

.task-chip--warning {
  color: var(--ws-color-warning);
  border-color: var(--ws-color-warning);
}

.task-chip--danger {
  color: var(--ws-color-danger);
  border-color: var(--ws-color-danger);
}

.task-chip--muted {
  color: var(--ws-text-disabled);
  border-color: var(--ws-border);
}
</style>
