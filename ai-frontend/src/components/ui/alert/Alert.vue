<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Feedback/Alert" node=13:34149 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { X } from "lucide-vue-next";
import { CircleAlert, CircleCheck, Info, TriangleAlert } from "lucide-vue-next";
import { cn } from "@/lib/cn";

export type AlertVariant = "info" | "success" | "warning" | "danger";
export type AlertSize = "sm" | "md" | "lg";

interface Props {
  /** 语义变体 —— Figma：info/success/warning 为中性面，仅 error 用 danger 面 */
  variant?: AlertVariant;
  /** 尺寸档 —— Figma：sm padding 8 / md 12 / lg 16 */
  size?: AlertSize;
  /** 标题 */
  title?: string;
  /** 可关闭：右侧展示 X（Figma Dismissible 态） */
  dismissible?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  variant: "info",
  size: "md",
  title: undefined,
  dismissible: false
});

const emit = defineEmits<{ close: [] }>();

/** Figma 反馈框为「图标 + 标题/正文」结构（图标 16px，圆角 8） */
const ICONS = {
  info: Info,
  success: CircleCheck,
  warning: TriangleAlert,
  danger: CircleAlert
} as const;

const icon = computed(() => ICONS[props.variant]);
</script>

<template>
  <div
    :class="cn('ui-alert', `ui-alert--${variant}`, `ui-alert--${size}`)"
    role="note"
  >
    <component :is="icon" class="ui-alert__icon" aria-hidden="true" />
    <div class="ui-alert__body">
      <p v-if="title" class="ui-alert__title">{{ title }}</p>
      <div class="ui-alert__content">
        <slot />
      </div>
    </div>
    <button
      v-if="dismissible"
      type="button"
      class="ui-alert__close"
      aria-label="Close"
      @click="emit('close')"
    >
      <X class="ui-alert__close-icon" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
/* Figma DS/Feedback/Alert（13:34149）+ 登录页反馈框（5:2505）：
 * padding sm8/md12/lg16 / gap 8 / radius 8 / 图标 16px；
 * info/success/warning = 中性面（elevated + border）；
 * error = danger 面 #2C2028 + 描边 #E69A9F（--danger-title）；
 * 标题 13 Medium（error 标题 #E69A9F），正文 11 Regular 次文字。 */
.ui-alert {
  display: flex;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}

.ui-alert--sm {
  padding: var(--space-2);
}

.ui-alert--lg {
  padding: var(--space-4);
}

.ui-alert--danger {
  background: var(--danger-surface);
  border-color: var(--danger-title);
}

.ui-alert__icon {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--info);
}

.ui-alert--success .ui-alert__icon {
  color: var(--success);
}

.ui-alert--warning .ui-alert__icon {
  color: var(--warning);
}

.ui-alert--danger .ui-alert__icon {
  color: var(--danger-title);
}

.ui-alert__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  flex: 1;
}

.ui-alert__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}

.ui-alert--danger .ui-alert__title {
  color: var(--danger-title);
}

.ui-alert__content {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

/* 可关闭 —— Figma Dismissible：X 图标 14px */
.ui-alert__close {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  align-self: flex-start;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: var(--radius-xs);
}

.ui-alert__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-alert__close-icon {
  width: var(--space-3);
  height: var(--space-3);
}
</style>
