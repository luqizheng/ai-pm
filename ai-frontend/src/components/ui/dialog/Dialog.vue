<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Overlays/Modal" node=13:33764 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import {
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle
} from "reka-ui";
import { useI18n } from "vue-i18n";

interface Props {
  /** 是否打开（v-model:open） */
  open?: boolean;
  /** 弹框标题 */
  title?: string;
  /** 副标题 / 说明 */
  description?: string;
  /** 关闭按钮的无障碍名称 */
  closeLabel?: string;
  /** 宽度档位 */
  size?: "sm" | "md" | "lg";
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
  title: undefined,
  description: undefined,
  closeLabel: undefined,
  size: "md"
});

const { t } = useI18n();

// 默认文案走 i18n（调用方显式传入时优先）
const closeLabelText = computed(() => props.closeLabel ?? t("common.close"));

const emit = defineEmits<{ "update:open": [value: boolean] }>();
</script>

<template>
  <DialogRoot :open="props.open" @update:open="emit('update:open', $event)">
    <DialogPortal>
      <DialogOverlay class="ui-dialog__overlay" />
      <DialogContent
        class="ui-dialog__content"
        :class="`ui-dialog--${props.size}`"
      >
        <header class="ui-dialog__header">
          <DialogTitle class="ui-dialog__title">{{ props.title }}</DialogTitle>
          <DialogClose class="ui-dialog__close" :aria-label="closeLabelText">
            <slot name="close">×</slot>
          </DialogClose>
        </header>
        <DialogDescription
          v-if="props.description"
          class="ui-dialog__description"
        >
          {{ props.description }}
        </DialogDescription>
        <div class="ui-dialog__body">
          <slot />
        </div>
        <footer v-if="$slots.footer" class="ui-dialog__footer">
          <slot name="footer" />
        </footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
/* 遮罩 — Phase 2 §4.2：暖黑纱压灰不压黑 */
.ui-dialog__overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: var(--surface-ink);
  opacity: 0.28;
  animation: ui-dialog-overlay-in var(--duration-medium) var(--ease-decelerate);
}

/* 浮层 — Figma DS/Overlays/Modal（13:33764）：
 * surface 底 + border 描边 + radius 8 / 宽三档 352/448/496 /
 * header·body·footer 统一 padding 16，header/footer 各带 1px 分隔线。 */
.ui-dialog__content {
  position: fixed;
  top: 50%;
  left: 50%;
  z-index: 51;
  display: flex;
  flex-direction: column;
  width: calc(100% - var(--space-8));
  max-height: calc(100vh - var(--space-16));
  transform: translate(-50%, -50%);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  /* 深色板 Figma 无投影（token=none）；浅色板恢复轻量投影 */
  box-shadow: var(--shadow-lg);
  animation: ui-dialog-in var(--duration-slow) var(--ease-decelerate);
}

/* Figma 宽度三档（Small 352 / Medium 448 / Large 496） */
.ui-dialog--sm {
  max-width: var(--size-overlay-panel-sm);
}

.ui-dialog--md {
  max-width: var(--size-overlay-panel);
}

.ui-dialog--lg {
  max-width: var(--size-overlay-panel-lg);
}

.ui-dialog__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4);
  border-bottom: var(--border-1) solid var(--border-strong);
}

.ui-dialog__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-body-strong);
  color: var(--text-primary);
}

.ui-dialog__description {
  margin: 0;
  padding: var(--space-4) var(--space-4) 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-secondary);
}

.ui-dialog__body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  overflow: auto;
}

.ui-dialog__footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-4);
  border-top: var(--border-1) solid var(--border-strong);
}

.ui-dialog__close {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--font-size-body);
  line-height: 1;
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-dialog__close:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-dialog__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

@keyframes ui-dialog-overlay-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 0.28;
  }
}

@keyframes ui-dialog-in {
  from {
    opacity: 0;
    transform: translate(-50%, calc(-50% + var(--space-2)));
  }
  to {
    opacity: 1;
    transform: translate(-50%, -50%);
  }
}

/* 减少动效 — Phase 2 §5.4：仅保留 opacity */
@media (prefers-reduced-motion: reduce) {
  .ui-dialog__overlay,
  .ui-dialog__content {
    animation: none;
  }
}
</style>
