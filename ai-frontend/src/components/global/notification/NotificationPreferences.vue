<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Notification/Preferences" node=33:85983 date=2026-11-04 -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";

import Switch from "@/components/ui/switch/Switch.vue";
import { NOTIFICATION_CATEGORIES, type NotificationCategory } from "./types";

/**
 * 通知偏好（Figma Global/Notification/Preferences 33:85983）。
 *
 * 按事件类别的送达开关（六类各一个 Switch）。
 * Figma 帧注记：**不含通道与路由配置**（No channel or routing configuration）。
 */
interface Props {
  /** 各类别送达开关（受控） */
  preferences: Record<NotificationCategory, boolean>;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  toggle: [category: NotificationCategory, value: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <section
    class="notification-preferences"
    :aria-label="t('global.notification.preferencesTitle')"
  >
    <h2 class="notification-preferences__title">
      {{ t("global.notification.preferencesTitle") }}
    </h2>
    <p class="notification-preferences__desc">
      {{ t("global.notification.preferencesDesc") }}
    </p>
    <Switch
      v-for="category in NOTIFICATION_CATEGORIES"
      :key="category"
      :model-value="props.preferences[category]"
      :label="t(`global.notification.category.${category}`)"
      @update:model-value="value => emit('toggle', category, value)"
    />
  </section>
</template>

<style scoped>
.notification-preferences {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.notification-preferences__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.notification-preferences__desc {
  margin: 0 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}
</style>
