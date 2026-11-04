<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Navigation/SidebarItem" node=13:33329 date=2026-11-04 -->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { LoaderCircle } from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * SidebarItem —— Figma DS/Navigation/SidebarItem（13:33329）。
 * 展开态：36px 行高 + 图标 16 + 文本 + 右侧徽标；
 * Active：accent 着色面底 + 左侧 2×16 accent 指示条（radius 1px）；
 * Focus：2px accent 描边；Restricted：次文字 + 禁用光标；Loading：loader 图标；
 * Collapsed：仅图标（文本隐藏）。
 */

export type SidebarItemSize = "sm" | "md" | "lg";
export type SidebarItemState =
  | "default"
  | "hover"
  | "active"
  | "focus"
  | "restricted"
  | "loading";

interface Props {
  label: string;
  icon?: Component;
  size?: SidebarItemSize;
  state?: SidebarItemState;
  collapsed?: boolean;
  /** 徽标内容（数字计数或短状态文字） */
  badge?: string | number;
  /** 徽标类型：count = 计数样式；status = 状态文字样式 */
  badgeType?: "count" | "status";
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  icon: undefined,
  size: "md",
  state: "default",
  collapsed: false,
  badge: undefined,
  badgeType: "count",
  disabled: false
});

const emit = defineEmits<{ click: [event: MouseEvent] }>();

const isActive = computed(() => props.state === "active");
const isRestricted = computed(() => props.state === "restricted");
const isLoading = computed(() => props.state === "loading");
const isDisabled = computed(() => props.disabled || isRestricted.value);
</script>

<template>
  <button
    type="button"
    :class="
      cn('ui-sidebar-item', `ui-sidebar-item--${size}`, {
        'is-active': isActive,
        'is-collapsed': collapsed,
        'is-restricted': isRestricted,
        'is-disabled': disabled
      })
    "
    :aria-current="isActive ? 'page' : undefined"
    :aria-busy="isLoading ? 'true' : undefined"
    :disabled="isDisabled"
    @click="emit('click', $event)"
  >
    <span
      v-if="isActive"
      class="ui-sidebar-item__indicator"
      aria-hidden="true"
    />
    <LoaderCircle
      v-if="isLoading"
      class="ui-sidebar-item__spinner"
      aria-hidden="true"
    />
    <component
      :is="icon"
      v-else-if="icon"
      class="ui-sidebar-item__icon"
      aria-hidden="true"
    />
    <span v-if="!collapsed" class="ui-sidebar-item__label">{{ label }}</span>
    <span
      v-if="!collapsed && badge !== undefined"
      :class="cn('ui-sidebar-item__badge', `is-${badgeType}`)"
    >
      {{ badge }}
    </span>
  </button>
</template>

<style scoped>
.ui-sidebar-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: 0 var(--space-3);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  font-family: var(--font-ui);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-sidebar-item--sm {
  height: var(--size-control-sm);
}

.ui-sidebar-item--md {
  height: var(--size-nav-item);
}

.ui-sidebar-item--lg {
  height: var(--size-control-lg);
}

.ui-sidebar-item:hover:not(.is-disabled) {
  background-color: var(--surface-raised);
}

.ui-sidebar-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

/* Active：accent 着色面 + 主文字 */
.ui-sidebar-item.is-active {
  background-color: var(--accent-surface);
  color: var(--text-primary);
}

/* 左侧激活指示条：2×16 accent，radius 1px，垂直居中 */
.ui-sidebar-item__indicator {
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: var(--size-nav-indicator-width);
  height: var(--size-nav-indicator-height);
  border-radius: var(--radius-nav-indicator);
  background-color: var(--accent);
}

/* Restricted：次文字 + 禁用光标（不置灰，保语义可读） */
.ui-sidebar-item.is-restricted {
  color: var(--text-secondary);
  cursor: not-allowed;
}

.ui-sidebar-item.is-disabled:not(.is-restricted) {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}

.ui-sidebar-item.is-collapsed {
  justify-content: center;
  padding: 0;
}

.ui-sidebar-item__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
}

.ui-sidebar-item__spinner {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  animation: ui-sidebar-item-spin var(--duration-spinner) linear infinite;
}

@keyframes ui-sidebar-item-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ui-sidebar-item__spinner {
    animation: none;
  }
}

.ui-sidebar-item__label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: inherit;
}

.ui-sidebar-item__badge {
  display: inline-flex;
  align-items: center;
  padding: 1px var(--space-1);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-primary);
}

/* 状态徽标：无底色，次文字 */
.ui-sidebar-item__badge.is-status {
  padding: 0;
  background: transparent;
  color: var(--text-secondary);
}
</style>
