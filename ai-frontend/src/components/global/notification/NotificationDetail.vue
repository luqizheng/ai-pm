<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Notification/Detail" node=33:87791 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import Button from "@/components/ui/button/Button.vue";
import HumanApprovalIndicator from "./HumanApprovalIndicator.vue";
import type { ApprovalIndicatorState, NotificationRecord } from "./types";

/**
 * 通知详情（Figma Global/Notification/Detail 33:87791）。
 *
 * 结构 = 标题行 + mono 分类 + 元数据行 + 分隔线 + 主题（15 SB）+
 * 上下文两行 + 人工批准指示器（可选）+ 操作行
 * （Open 对象 / 处理请求·仅待决 / 标记已读·仅未读）。
 * 审批三态（Pending/Approved/Rejected）经 HumanApprovalIndicator 呈现，
 * **永不暗示 AI 建议已被人类批准**。
 */
interface Props {
  record: NotificationRecord;
  /** 人工批准态（审批类通知）；缺省不渲染指示器 */
  approval?: { state: ApprovalIndicatorState; actor?: string };
}

const props = defineProps<Props>();

const emit = defineEmits<{
  open: [record: NotificationRecord];
  review: [record: NotificationRecord];
  "mark-read": [record: NotificationRecord];
}>();

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
  <section
    class="notification-detail"
    :aria-label="t('global.notification.detailLabel')"
  >
    <h2 class="notification-detail__heading">
      {{ t("global.notification.detailLabel") }}
    </h2>
    <p class="notification-detail__category">
      {{ t(`global.notification.category.${props.record.category}`) }}
    </p>
    <p class="notification-detail__meta">{{ metaLine }}</p>

    <div class="notification-detail__divider" aria-hidden="true" />

    <h3 class="notification-detail__subject">{{ props.record.title }}</h3>
    <p class="notification-detail__context">{{ props.record.object }}</p>
    <p class="notification-detail__context">{{ props.record.description }}</p>

    <HumanApprovalIndicator
      v-if="props.approval"
      :state="props.approval.state"
      :actor="props.approval.actor"
    />

    <div class="notification-detail__actions">
      <Button variant="secondary" size="sm" @click="emit('open', props.record)">
        {{ t("global.notification.open", { object: props.record.openLabel }) }}
      </Button>
      <Button
        v-if="props.approval?.state === 'pending'"
        variant="primary"
        size="sm"
        @click="emit('review', props.record)"
      >
        {{ t("global.notification.reviewRequest") }}
      </Button>
      <Button
        v-if="!props.record.read"
        variant="ghost"
        size="sm"
        @click="emit('mark-read', props.record)"
      >
        {{ t("global.notification.markRead") }}
      </Button>
    </div>
  </section>
</template>

<style scoped>
.notification-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.notification-detail__heading {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.notification-detail__category {
  margin: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.notification-detail__meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

.notification-detail__divider {
  height: var(--hairline);
  background: var(--ws-border);
}

.notification-detail__subject {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.notification-detail__context {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.notification-detail__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}
</style>
