<script setup lang="ts">
/**
 * SettingsHelperText —— 字段/分区辅助说明文字（Settings Framework §7）。
 * 元文案（micro 字阶、次文字色）；与错误态互斥时由调用方择一渲染。
 */
defineProps<{
  /** 说明文案（已翻译）；不传时走插槽 */
  text?: string;
}>();
</script>

<template>
  <p class="settings-helper-text">
    <slot>{{ text }}</slot>
  </p>
</template>

<style scoped>
.settings-helper-text {
  margin: 0;
  font-size: var(--ws-font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}
</style>
