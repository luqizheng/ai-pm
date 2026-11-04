<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Data/MenuItem" node=13:34522 date=2026-11-04 -->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { cn } from "@/lib/cn";

/**
 * MenuItem —— Figma DS/Data/MenuItem（13:34522）。
 * 36px 行 / radius 8 / 图标 16 + 标签 + 右端快捷键 chip；
 * 选中/焦点项底 accent 着色面 + 2px accent 描边；禁用项 opacity 0.35；
 * danger 变体走 danger 语义色。
 */

export type MenuItemSize = "sm" | "md" | "lg";

interface Props {
  label: string;
  icon?: Component;
  size?: MenuItemSize;
  shortcut?: string;
  selected?: boolean;
  disabled?: boolean;
  danger?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  icon: undefined,
  size: "md",
  shortcut: "",
  selected: false,
  disabled: false,
  danger: false
});

const emit = defineEmits<{ click: [event: MouseEvent] }>();

const hasShortcut = computed(() => props.shortcut.length > 0);
</script>

<template>
  <button
    type="button"
    :class="
      cn('ui-menu-item', `ui-menu-item--${size}`, {
        'is-selected': selected,
        'is-disabled': disabled,
        'is-danger': danger
      })
    "
    :aria-selected="selected ? 'true' : undefined"
    :disabled="disabled"
    @click="emit('click', $event)"
  >
    <component
      :is="icon"
      v-if="icon"
      class="ui-menu-item__icon"
      aria-hidden="true"
    />
    <span class="ui-menu-item__label">{{ label }}</span>
    <kbd v-if="hasShortcut" class="ui-menu-item__shortcut">{{ shortcut }}</kbd>
  </button>
</template>

<style scoped>
.ui-menu-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: 0 var(--space-3);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  text-align: left;
  cursor: pointer;
  transition: background-color var(--duration-fast) var(--ease-decelerate);
}

.ui-menu-item--sm {
  height: var(--size-control-sm);
}

.ui-menu-item--md {
  height: var(--size-nav-item);
}

.ui-menu-item--lg {
  height: var(--size-control-lg);
}

.ui-menu-item:hover:not(.is-disabled) {
  background-color: var(--surface-raised);
}

.ui-menu-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-menu-item.is-selected {
  background-color: var(--accent-surface);
}

.ui-menu-item.is-selected:focus-visible {
  outline-color: var(--accent);
}

.ui-menu-item.is-disabled {
  opacity: var(--opacity-disabled-option);
  cursor: not-allowed;
}

.ui-menu-item__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

.ui-menu-item.is-danger {
  color: var(--danger-title);
}

.ui-menu-item.is-danger .ui-menu-item__icon {
  color: var(--danger-title);
}

.ui-menu-item__label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: inherit;
}

.ui-menu-item__shortcut {
  flex: none;
  padding: 1px var(--space-1);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
}
</style>
