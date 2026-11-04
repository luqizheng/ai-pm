<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/AI/HumanApprovalIndicator" node=13:34834 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { ShieldCheck, ShieldQuestion, ShieldX } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import type { ApprovalIndicatorState } from "./types";

/**
 * 人工批准指示器（DS/AI/HumanApprovalIndicator 13:34834）。
 *
 * 三态：Pending（accent 描边 + shield-question）/ Approved（shield-check）/
 * Rejected（shield-x）。
 * 设计红线：**永不暗示 AI 建议已被人类批准**——Approved 态文案显式声明
 * 「非 AI 批准」（Not AI approval），Rejected 态显式声明「未授权任何变更」。
 */
interface Props {
  state: ApprovalIndicatorState;
  /** 决策人显示名（approved/rejected 态的说明文案） */
  actor?: string;
}

const props = withDefaults(defineProps<Props>(), {
  actor: undefined
});

const { t } = useI18n();

const ICONS = {
  pending: ShieldQuestion,
  approved: ShieldCheck,
  rejected: ShieldX
} as const;

const icon = computed(() => ICONS[props.state]);

const title = computed(() =>
  t(`global.notification.approval.${props.state}.title`)
);

const desc = computed(() => {
  if (props.state === "pending") {
    return t("global.notification.approval.pending.desc");
  }
  return t(`global.notification.approval.${props.state}.desc`, {
    actor: props.actor ?? ""
  });
});
</script>

<template>
  <div class="approval-indicator" :class="`is-${props.state}`" role="status">
    <component
      :is="icon"
      :size="16"
      class="approval-indicator__icon"
      aria-hidden="true"
    />
    <div class="approval-indicator__body">
      <p class="approval-indicator__title">{{ title }}</p>
      <p class="approval-indicator__desc">{{ desc }}</p>
    </div>
  </div>
</template>

<style scoped>
.approval-indicator {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

/* Pending：accent 描边（等待人工决策） */
.approval-indicator.is-pending {
  border-color: var(--ws-accent);
}

.approval-indicator__icon {
  flex: none;
  color: var(--ws-text-secondary);
}

.approval-indicator.is-pending .approval-indicator__icon {
  color: var(--ws-accent);
}

.approval-indicator.is-approved .approval-indicator__icon {
  color: var(--ws-color-success);
}

.approval-indicator.is-rejected .approval-indicator__icon {
  color: var(--ws-color-danger);
}

.approval-indicator__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.approval-indicator__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  line-height: var(--line-height-body);
}

.approval-indicator__desc {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}
</style>
