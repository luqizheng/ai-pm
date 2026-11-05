<script setup lang="ts">
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { TaskStat } from "../task-overview";

/**
 * 任务概览的单张迷你统计卡（Command Center 页内私有布局件）。
 *
 * 结构（Pixso「Project Command Center」3:7377 系）：
 *   标签行（标签 + 单位右对齐）→ 大数字 → 明细行 → 口径脚注。
 * 样式约束：颜色/圆角/间距全部走 token；语义点用公共 StatusIndicator，
 * 不自造色点；数据本身不负责翻译。
 */
defineProps<TaskStat>();
</script>

<template>
  <div class="pm-task-stat">
    <div class="pm-task-stat__head">
      <span class="pm-task-stat__label">{{ label }}</span>
      <span v-if="unit" class="pm-task-stat__unit">{{ unit }}</span>
    </div>
    <span class="pm-task-stat__count">{{ count }}</span>
    <div v-if="breakdown?.length" class="pm-task-stat__rows">
      <div v-for="row in breakdown" :key="row.label" class="pm-task-stat__row">
        <StatusIndicator :status="row.tone" size="sm" :label="row.label" />
        <span class="pm-task-stat__row-count">{{ row.count }}</span>
      </div>
    </div>
    <span v-if="hint" class="pm-task-stat__hint">{{ hint }}</span>
  </div>
</template>

<style scoped>
.pm-task-stat {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  padding: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}

.pm-task-stat__head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
  min-width: 0;
}

.pm-task-stat__label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--text-secondary);
}

.pm-task-stat__unit {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.pm-task-stat__count {
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h3);
  font-variant-numeric: tabular-nums;
  color: var(--text-primary);
}

.pm-task-stat__rows {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.pm-task-stat__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  min-width: 0;
}

.pm-task-stat__row-count {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  font-variant-numeric: tabular-nums;
  color: var(--text-primary);
}

.pm-task-stat__hint {
  font-size: var(--font-size-nano);
  line-height: 1.4;
  color: var(--text-disabled);
}
</style>
