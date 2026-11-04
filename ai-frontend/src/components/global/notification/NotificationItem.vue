<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Data/NotificationItem" node=13:34472 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { MessageSquare } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import type { NotificationRecord } from "./types";

/**
 * 通知条目（DS/Data/NotificationItem 13:34472）。
 *
 * Unread：accent 着色面 + 标题 13 Semi Bold + 6px 未读点；
 * Read：surface 底 + 标题 13 Regular，无点。
 * 结构 = 图标（message-square 16）+ 内容列（标题 / 11px 描述 / 11px 元数据行），
 * 元数据行 =「未读·时间 · 对象」，状态词走 i18n。
 */
interface Props {
  record: NotificationRecord;
}

const props = defineProps<Props>();

const { t } = useI18n();

const metaLine = computed(() =>
  [
    props.record.read
      ? t("global.notification.statusRead")
      : t("global.notification.statusUnread"),
    props.record.timestamp,
    props.record.object
  ].join(" · ")
);
</script>

<template>
  <div
    class="notification-item"
    :class="props.record.read ? 'is-read' : 'is-unread'"
  >
    <MessageSquare :size="16" class="notification-item__icon" aria-hidden="true" />
    <div class="notification-item__content">
      <p class="notification-item__title">{{ props.record.title }}</p>
      <p class="notification-item__desc">{{ props.record.description }}</p>
      <p class="notification-item__meta">{{ metaLine }}</p>
    </div>
    <span
      v-if="!props.record.read"
      class="notification-item__dot"
      aria-hidden="true"
    />
  </div>
</template>

<style scoped>
.notification-item {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-3);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

/* Unread：accent 着色面（Figma #242238 / 浅色 #EEEBFF） */
.notification-item.is-unread {
  background: var(--ws-color-accent-surface);
}

.notification-item.is-read {
  background: var(--ws-surface);
}

.notification-item__icon {
  flex: none;
  margin-top: 2px;
  color: var(--ws-text-secondary);
}

.notification-item__content {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1;
  min-width: 0;
}

.notification-item__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

/* 未读标题 Semi Bold，已读 Regular（Figma Item Unread/Read 差异） */
.notification-item.is-unread .notification-item__title {
  font-weight: var(--weight-semibold);
}

.notification-item__desc,
.notification-item__meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.notification-item__dot {
  position: absolute;
  top: var(--space-3);
  right: var(--space-3);
  width: 6px;
  height: 6px;
  background: var(--ws-accent);
  border-radius: var(--radius-full);
}
</style>
