<script setup lang="ts">
import SettingsSectionHeader from "./SettingsSectionHeader.vue";

/**
 * SettingsFormSection —— 设置表单分区容器（Settings Framework §7）。
 * 组合 SettingsSectionHeader + 表单行插槽；不承载任何业务状态。
 */
defineProps<{
  /** 分区标题（已翻译；空省略 = 无头分区，仅排字段行） */
  title?: string;
  /** 分区描述（已翻译；空省略） */
  description?: string;
}>();
</script>

<template>
  <section class="settings-form-section">
    <SettingsSectionHeader
      v-if="title"
      :title="title"
      :description="description"
    >
      <template v-if="$slots.actions" #actions>
        <slot name="actions" />
      </template>
    </SettingsSectionHeader>
    <div class="settings-form-section__body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.settings-form-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.settings-form-section__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}
</style>
