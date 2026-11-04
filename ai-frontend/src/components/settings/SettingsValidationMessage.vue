<script setup lang="ts">
import { CircleAlert } from "lucide-vue-next";

/**
 * SettingsValidationMessage —— 字段校验错误文案（Settings Framework §6.5/§7）。
 * role=alert 保证出现即播报；视觉走 danger 语义色，与 ui/input 的 invalid 态同源。
 */
defineProps<{
  /** 错误文案（已翻译） */
  message: string;
}>();
</script>

<template>
  <p class="settings-validation" role="alert">
    <CircleAlert class="settings-validation__icon" aria-hidden="true" />
    <span>{{ message }}</span>
  </p>
</template>

<style scoped>
.settings-validation {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
  font-size: var(--ws-font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-color-danger-text);
}

.settings-validation__icon {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}
</style>
