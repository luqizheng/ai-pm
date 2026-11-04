<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Data/MenuItem" node=13:34522 date=2026-11-04 -->
<script setup lang="ts">
import { Check } from "lucide-vue-next";

/**
 * 菜单行（DS/Data/MenuItem 13:34522，本地实现）。
 *
 * h36 / padding 0 8 / radius 8 / 标签 13；hover 薄纱；
 * selected = accent 着色面 + 右侧 check（Appearance 选中项）；
 * danger = 标签 danger-title 色（退出登录）；disabled 整体 0.4。
 */
interface Props {
  label: string;
  variant?: "default" | "danger" | "selected";
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  variant: "default",
  disabled: false
});

const emit = defineEmits<{ click: [] }>();
</script>

<template>
  <button
    type="button"
    class="user-menu-item"
    :class="[`is-${props.variant}`, { 'is-disabled': props.disabled }]"
    role="menuitem"
    :disabled="props.disabled"
    @click="emit('click')"
  >
    <span class="user-menu-item__label">{{ props.label }}</span>
    <Check
      v-if="props.variant === 'selected'"
      :size="14"
      class="user-menu-item__check"
      aria-hidden="true"
    />
  </button>
</template>

<style scoped>
.user-menu-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: var(--ws-row-height);
  padding: 0 var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.user-menu-item:hover:not(.is-disabled) {
  background: var(--ws-hover-veil);
}

.user-menu-item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* Selected：accent 着色面（Figma MenuItem State=Selected） */
.user-menu-item.is-selected {
  background: var(--ws-color-accent-surface);
}

/* Danger：退出登录（danger-title 文字色） */
.user-menu-item.is-danger {
  color: var(--ws-color-danger-title);
}

.user-menu-item.is-disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-option);
}

.user-menu-item__label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.user-menu-item__check {
  flex: none;
  color: var(--ws-text-primary);
}
</style>
