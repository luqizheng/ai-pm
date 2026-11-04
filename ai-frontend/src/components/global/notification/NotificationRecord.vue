<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Notification/Record" node=33:87784 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import NotificationItem from "./NotificationItem.vue";
import type { NotificationRecord } from "./types";

/**
 * 通知记录块（Figma Global/Notification/Record 33:87784）。
 *
 * 结构 = 分类行（mono 11 类别 + 可选 Human attention 徽标）+
 * NotificationItem + 对象操作行（Open X / Mark read|unread）。
 * Selected 态（通知中心）：accent 着色面 + accent 描边。
 */
interface Props {
  record: NotificationRecord;
  /** 中心列表中的选中态 */
  selected?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  selected: false
});

const emit = defineEmits<{
  select: [record: NotificationRecord];
  open: [record: NotificationRecord];
  "toggle-read": [record: NotificationRecord];
}>();

const { t } = useI18n();

const categoryLabel = computed(() =>
  t(`global.notification.category.${props.record.category}`)
);

const openLabel = computed(() =>
  t("global.notification.open", { object: props.record.openLabel })
);
</script>

<template>
  <div
    class="notification-record"
    :class="{ 'is-selected': props.selected }"
  >
    <div class="notification-record__category-row">
      <span class="notification-record__category">{{ categoryLabel }}</span>
      <Badge v-if="props.record.requiresAttention" variant="accent" size="sm">
        {{ t("global.notification.attention") }}
      </Badge>
    </div>

    <NotificationItem :record="props.record" @click="emit('select', props.record)" />

    <div class="notification-record__actions">
      <Button variant="ghost" size="sm" @click="emit('open', props.record)">
        {{ openLabel }}
      </Button>
      <Button variant="ghost" size="sm" @click="emit('toggle-read', props.record)">
        {{
          props.record.read
            ? t("global.notification.markUnread")
            : t("global.notification.markRead")
        }}
      </Button>
    </div>
  </div>
</template>

<style scoped>
.notification-record {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2);
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-sm);
}

/* Selected（通知中心）：accent 着色面 + accent 描边 */
.notification-record.is-selected {
  background: var(--ws-color-accent-surface);
  border-color: var(--ws-accent);
}

.notification-record__category-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.notification-record__category {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.notification-record__actions {
  display: flex;
  gap: var(--space-2);
}
</style>
