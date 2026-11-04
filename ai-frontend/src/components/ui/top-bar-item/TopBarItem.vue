<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Navigation/TopBarItem" node=13:33520 date=2026-11-04 -->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { cn } from "@/lib/cn";

/**
 * TopBarItem —— Figma DS/Navigation/TopBarItem（13:33520）。
 * 顶栏条目：图标 16 + 标签 + 可选徽标 + 右端快捷键 chip（mono 10px）；
 * 36px 行高 / radius 8；Active 走 accent 着色面。
 */

export type TopBarItemSize = "sm" | "md" | "lg";

interface Props {
  label: string;
  icon?: Component;
  size?: TopBarItemSize;
  active?: boolean;
  disabled?: boolean;
  /** 键盘快捷键提示（如 ⌘ K）；空串隐藏 */
  shortcut?: string;
  badge?: string | number;
}

const props = withDefaults(defineProps<Props>(), {
  icon: undefined,
  size: "md",
  active: false,
  disabled: false,
  shortcut: "",
  badge: undefined
});

const emit = defineEmits<{ click: [event: MouseEvent] }>();

const hasShortcut = computed(() => props.shortcut.length > 0);
</script>

<template>
  <button
    type="button"
    :class="
      cn('ui-topbar-item', `ui-topbar-item--${size}`, {
        'is-active': active,
        'is-disabled': disabled
      })
    "
    :aria-current="active ? 'page' : undefined"
    :disabled="disabled"
    @click="emit('click', $event)"
  >
    <component
      :is="icon"
      v-if="icon"
      class="ui-topbar-item__icon"
      aria-hidden="true"
    />
    <span class="ui-topbar-item__label">{{ label }}</span>
    <span v-if="badge !== undefined" class="ui-topbar-item__badge">{{
      badge
    }}</span>
    <kbd v-if="hasShortcut" class="ui-topbar-item__shortcut">{{
      shortcut
    }}</kbd>
  </button>
</template>

<style scoped>
.ui-topbar-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  border: 0;
  border-radius: var(--radius-sm);
  background-color: var(--surface-sunken);
  font-family: var(--font-ui);
  cursor: pointer;
  transition: background-color var(--duration-fast) var(--ease-decelerate);
}

.ui-topbar-item--sm {
  height: var(--size-control-sm);
}

.ui-topbar-item--md {
  height: var(--size-nav-item);
}

.ui-topbar-item--lg {
  height: var(--size-control-lg);
}

.ui-topbar-item:hover:not(.is-disabled) {
  background-color: var(--surface-raised);
}

.ui-topbar-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-topbar-item.is-active {
  background-color: var(--accent-surface);
}

.ui-topbar-item.is-disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}

.ui-topbar-item__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

.ui-topbar-item__label {
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
  white-space: nowrap;
}

.ui-topbar-item__badge {
  display: inline-flex;
  align-items: center;
  padding: 1px var(--space-1);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-primary);
}

.ui-topbar-item__shortcut {
  padding: 1px var(--space-1);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
}
</style>
