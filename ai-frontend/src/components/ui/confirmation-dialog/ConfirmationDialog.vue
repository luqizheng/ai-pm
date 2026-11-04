<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Overlays/ConfirmationDialog" node=13:33877 date=2026-11-04 -->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { cn } from "@/lib/cn";

/**
 * ConfirmationDialog —— Figma DS/Overlays/ConfirmationDialog（13:33877）。
 * 448px 面板：header（图标 + 标题，下边框）→ body（说明 + 确认勾选）→
 * footer（右对齐：取消 secondary + 确认 primary/danger）。
 * Figma 源异常备注：Danger 变体按钮文案仍写 Archive item（疑似模板复用错误），
 * 本实现按语义由 props 传入文案，不做硬编码。
 */

export type ConfirmationDialogSize = "sm" | "md" | "lg";

interface Props {
  open: boolean;
  title: string;
  description?: string;
  confirmLabel: string;
  cancelLabel: string;
  /** danger = 破坏性确认（按钮走 danger 面） */
  variant?: "default" | "danger";
  size?: ConfirmationDialogSize;
  /** body 内确认勾选文案（如 I have reviewed this change.）；空串隐藏 */
  checkboxLabel?: string;
  icon?: Component;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  description: undefined,
  variant: "default",
  size: "md",
  checkboxLabel: "",
  icon: undefined,
  disabled: false
});

const emit = defineEmits<{
  "update:open": [value: boolean];
  confirm: [];
  cancel: [];
}>();

const showCheckbox = computed(() => props.checkboxLabel.length > 0);
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="ui-confirm" role="presentation">
      <div
        :class="
          cn('ui-confirm__panel', `ui-confirm__panel--${size}`, {
            'is-danger': variant === 'danger'
          })
        "
        role="alertdialog"
        aria-modal="true"
        :aria-label="title"
      >
        <header class="ui-confirm__header">
          <component
            :is="icon"
            v-if="icon"
            class="ui-confirm__icon"
            aria-hidden="true"
          />
          <h2 class="ui-confirm__title">{{ title }}</h2>
        </header>

        <div class="ui-confirm__body">
          <p v-if="description" class="ui-confirm__description">
            {{ description }}
          </p>
          <label v-if="showCheckbox" class="ui-confirm__checkbox">
            <input v-if="showCheckbox" type="checkbox" />
            <span>{{ checkboxLabel }}</span>
          </label>
        </div>

        <footer class="ui-confirm__footer">
          <button
            type="button"
            class="ui-confirm__btn ui-confirm__btn--cancel"
            :disabled="disabled"
            @click="
              emit('update:open', false);
              emit('cancel');
            "
          >
            {{ cancelLabel }}
          </button>
          <button
            type="button"
            :class="
              cn('ui-confirm__btn', {
                'ui-confirm__btn--danger': variant === 'danger',
                'ui-confirm__btn--primary': variant !== 'danger'
              })
            "
            :disabled="disabled"
            @click="emit('confirm')"
          >
            {{ confirmLabel }}
          </button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.ui-confirm {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--ws-hover-veil);
}

.ui-confirm__panel {
  display: flex;
  flex-direction: column;
  width: var(--size-overlay-panel);
  max-width: calc(100vw - var(--space-8));
  background-color: var(--surface-base);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
}

.ui-confirm__panel--sm {
  max-width: calc(var(--size-overlay-panel) - var(--space-12));
}

.ui-confirm__panel--lg {
  max-width: calc(var(--size-overlay-panel) + var(--space-12));
}

.ui-confirm__header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4);
  border-bottom: var(--border-1) solid var(--border-strong);
}

.ui-confirm__icon {
  width: var(--size-icon-md);
  height: var(--size-icon-md);
  flex: none;
  color: var(--text-secondary);
}

.ui-confirm__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ui-confirm__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
}

.ui-confirm__description {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
}

.ui-confirm__checkbox {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ui-confirm__footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-4);
  border-top: var(--border-1) solid var(--border-strong);
}

.ui-confirm__btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate);
}

.ui-confirm__btn:disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-button);
}

.ui-confirm__btn:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-confirm__btn--cancel {
  background-color: var(--surface-raised);
  border-color: var(--border-strong);
  color: var(--text-primary);
}

.ui-confirm__btn--cancel:hover:not(:disabled) {
  background-color: var(--ws-hover-veil);
}

.ui-confirm__btn--primary {
  background-color: var(--accent);
  color: var(--accent-contrast-text);
}

.ui-confirm__btn--primary:hover:not(:disabled) {
  background-color: var(--accent-hover);
}

.ui-confirm__btn--danger {
  background-color: var(--danger-surface);
  border-color: var(--danger-border);
  color: var(--danger-title);
}

.ui-confirm__btn--danger:hover:not(:disabled) {
  background-color: transparent;
}
</style>
