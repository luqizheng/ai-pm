<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { statusTone } from "../tasks-contract";

/**
 * 任务状态芯片（S04 私有展示件）。
 *
 * 色调映射 tasks-contract.statusTone：语义色以「描边 + 文字」呈现
 * （语义色基准对表面 ≥3:1，token 契约）；ready 走专用
 * --ws-color-status-ready，不用紫色系（V2 取消紫色强调）。
 */
const props = defineProps<{ status: string }>();

const { t } = useI18n();

const tone = computed(() => statusTone(props.status));
const label = computed(() => t(`s04.tasks.status.${props.status}`));
</script>

<template>
  <span
    class="task-chip"
    :class="`task-chip--${tone}`"
    :data-status="props.status"
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

.task-chip--ready {
  color: var(--ws-color-status-ready);
  border-color: var(--ws-color-status-ready);
}

.task-chip--warning {
  color: var(--ws-color-warning);
  border-color: var(--ws-color-warning);
}

.task-chip--danger {
  color: var(--ws-color-danger);
  border-color: var(--ws-color-danger);
}

.task-chip--success {
  color: var(--ws-color-success);
  border-color: var(--ws-color-success);
}

.task-chip--muted {
  color: var(--ws-text-disabled);
  border-color: var(--ws-border);
}
</style>
