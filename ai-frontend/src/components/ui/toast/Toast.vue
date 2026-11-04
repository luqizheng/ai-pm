<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Feedback/Toast" node=13:34097 date=2026-11-04 -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  CircleAlert,
  CircleCheck,
  Info,
  TriangleAlert,
  X
} from "lucide-vue-next";

export type ToastVariant = "success" | "info" | "warning" | "danger";
export type ToastSize = "sm" | "md" | "lg";

interface Props {
  /** 语义变体 —— Figma：info/success 中性面；error danger 面 */
  variant?: ToastVariant;
  /** 尺寸档 —— Figma：sm padding 8 / md 12 / lg 16 */
  size?: ToastSize;
  /** 标题 */
  title: string;
  /** 描述（Figma Toast 无正文；保留兼容，按 11px 渲染） */
  description?: string;
  /** 动作链接文案（Figma Action 态：Refresh / Undo / Retry，11px accent） */
  actionLabel?: string;
  /** 是否可关闭 */
  closable?: boolean;
  /** 关闭按钮的无障碍名称 */
  closeLabel?: string;
  /** 停留时长（毫秒），默认 4000 */
  duration?: number;
  /** 是否打开（v-model:open） */
  open?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  variant: "info",
  size: "md",
  description: undefined,
  actionLabel: undefined,
  closable: true,
  closeLabel: undefined,
  duration: 4000,
  open: true
});

const { t } = useI18n();

// 默认文案走 i18n（调用方显式传入时优先）
const closeLabelText = computed(() => props.closeLabel ?? t("common.close"));

const emit = defineEmits<{
  "update:open": [value: boolean];
  action: [];
}>();

/** success / info 不打断当前朗读；warning / danger 立即播报 */
const role = computed(() =>
  props.variant === "warning" || props.variant === "danger" ? "alert" : "status"
);

const ICONS = {
  info: Info,
  success: CircleCheck,
  warning: TriangleAlert,
  danger: CircleAlert
} as const;

const icon = computed(() => ICONS[props.variant]);

let timer: ReturnType<typeof setTimeout> | undefined;

function clearTimer() {
  if (timer !== undefined) {
    clearTimeout(timer);
    timer = undefined;
  }
}

function startTimer() {
  clearTimer();
  timer = setTimeout(() => {
    timer = undefined;
    emit("update:open", false);
  }, props.duration);
}

/** 悬停暂停计时 */
function pause() {
  clearTimer();
}

onMounted(() => {
  if (props.open) startTimer();
});

onBeforeUnmount(() => {
  clearTimer();
});

function close() {
  clearTimer();
  emit("update:open", false);
}
</script>

<template>
  <div class="ui-toast__viewport" @pointermove="pause">
    <div
      class="ui-toast"
      :class="[`ui-toast--${variant}`, `ui-toast--${size}`]"
      :role="role"
    >
      <component :is="icon" class="ui-toast__icon" aria-hidden="true" />
      <div class="ui-toast__content">
        <p class="ui-toast__title">{{ title }}</p>
        <p v-if="description" class="ui-toast__description">
          {{ description }}
        </p>
      </div>
      <button
        v-if="actionLabel"
        type="button"
        class="ui-toast__action"
        @click="emit('action')"
      >
        {{ actionLabel }}
      </button>
      <button
        v-if="closable"
        type="button"
        class="ui-toast__close"
        :aria-label="closeLabelText"
        @click="close"
      >
        <X class="ui-toast__close-icon" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>

<style scoped>
/* Figma DS/Feedback/Toast（13:34097）：
 * 中性面 elevated + border 描边 + radius 8 / 图标 16px（语义色）/
 * 标题 13 Medium（error 标题 #E69A9F）/ 动作链接 11 Medium accent / X 14px；
 * sm padding 8 / md 12 / lg 16。 */
.ui-toast__viewport {
  position: fixed;
  right: var(--space-6);
  bottom: var(--space-6);
  z-index: 60;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  pointer-events: none;
}

.ui-toast {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  min-width: 280px;
  max-width: 420px;
  padding: var(--space-3);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  pointer-events: auto;
  animation: ui-toast-enter var(--duration-medium) var(--ease-decelerate);
}

.ui-toast--sm {
  padding: var(--space-2);
}

.ui-toast--lg {
  padding: var(--space-4);
}

.ui-toast--danger {
  background: var(--danger-surface);
  border-color: var(--danger-title);
}

.ui-toast__icon {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--info);
}

.ui-toast--success .ui-toast__icon {
  color: var(--success);
}

.ui-toast--warning .ui-toast__icon {
  color: var(--warning);
}

.ui-toast--danger .ui-toast__icon {
  color: var(--danger-title);
}

.ui-toast__content {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1;
}

.ui-toast__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}

.ui-toast--danger .ui-toast__title {
  color: var(--danger-title);
}

.ui-toast__description {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
}

/* 动作链接 —— Figma Action 态：11px Medium accent */
.ui-toast__action {
  flex: none;
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
  color: var(--accent);
  cursor: pointer;
}

.ui-toast__action:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
  border-radius: var(--radius-xs);
}

.ui-toast__close {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-toast__close:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-toast__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-toast__close-icon {
  width: var(--space-3);
  height: var(--space-3);
}

@keyframes ui-toast-enter {
  from {
    opacity: 0;
    transform: translateY(var(--space-2));
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 窄屏：顶部居中 */
@media (max-width: 640px) {
  .ui-toast__viewport {
    right: 0;
    left: 0;
    bottom: auto;
    top: var(--space-4);
    align-items: center;
  }

  .ui-toast {
    max-width: calc(100% - var(--space-8));
  }
}

/* 减少动效 —— 仅保留 opacity */
@media (prefers-reduced-motion: reduce) {
  .ui-toast {
    animation: none;
  }
}
</style>
