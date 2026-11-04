<script setup lang="ts">
/**
 * SettingsSectionHeader —— 设置分区标题（Settings Framework §5/§7）。
 * 标题（section 字阶）+ 可选描述 + 右侧操作插槽；底部 hairline 分隔。
 */
defineProps<{
  /** 分区标题（已翻译） */
  title: string;
  /** 分区描述（已翻译；空省略） */
  description?: string;
}>();
</script>

<template>
  <header class="settings-section-header">
    <div class="settings-section-header__text">
      <h2 class="settings-section-header__title">{{ title }}</h2>
      <p v-if="description" class="settings-section-header__desc">
        {{ description }}
      </p>
    </div>
    <div v-if="$slots.actions" class="settings-section-header__actions">
      <slot name="actions" />
    </div>
  </header>
</template>

<style scoped>
.settings-section-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
  padding-bottom: var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.settings-section-header__text {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.settings-section-header__title {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.settings-section-header__desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.settings-section-header__actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
}
</style>
