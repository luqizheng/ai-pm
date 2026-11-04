<script setup lang="ts">
/**
 * SettingsReadOnlyField —— 只读字段展示（Settings Framework §6.8/§7）。
 *
 * 视觉上与可编辑输入框同构（同高、同描边）但呈凹陷只读面，
 * 明确区分「可编辑」与「不可改」——§6 要求可编辑/只读一眼可辨。
 * 文案过长时自动换行（租户 ID、系统生成值等场景）。
 */
defineProps<{
  /** 只读值（已格式化文本） */
  value: string;
  /** 值的无障碍名称（独立出现、旁边无标签时使用） */
  label?: string;
  /** 空值占位文案 */
  placeholder?: string;
}>();
</script>

<template>
  <div class="settings-readonly" :aria-label="label" aria-readonly="true">
    <span v-if="value" class="settings-readonly__value">{{ value }}</span>
    <span v-else class="settings-readonly__placeholder">{{
      placeholder ?? value
    }}</span>
  </div>
</template>

<style scoped>
.settings-readonly {
  display: flex;
  align-items: center;
  min-height: var(--size-control-md);
  padding: 0 var(--space-3);
  background-color: var(--surface-sunken);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.settings-readonly__value {
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-secondary);
  word-break: break-all;
  white-space: pre-wrap;
}

.settings-readonly__placeholder {
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-disabled);
}
</style>
