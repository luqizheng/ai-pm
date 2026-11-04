<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Data/AvatarGroup" node=13:34379 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";

/**
 * AvatarGroup —— Figma DS/Data/AvatarGroup（13:34379）。
 * 头像横向叠放（Figma gap: -8px → 负 margin 重叠）；超出的以计数徽标收口。
 * 复用 ui/avatar 组件（size 28/36/44 三档口径）。
 */

export type AvatarGroupSize = "sm" | "md" | "lg";

interface Props {
  /** 成员名数组（首字母 fallback） */
  avatars: string[];
  max?: number;
  size?: AvatarGroupSize;
}

const props = withDefaults(defineProps<Props>(), {
  max: 4,
  size: "sm"
});

const visible = computed(() => props.avatars.slice(0, props.max));
const overflow = computed(() => props.avatars.length - visible.value.length);

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map(part => part[0]!.toUpperCase())
    .join("");
}
</script>

<template>
  <div
    :class="['ui-avatar-group', `ui-avatar-group--${size}`]"
    :aria-label="`${avatars.length} members`"
  >
    <span
      v-for="(name, i) in visible"
      :key="`${name}-${i}`"
      class="ui-avatar-group__item"
    >
      <span class="ui-avatar-group__circle">{{ initials(name) }}</span>
    </span>
    <span v-if="overflow > 0" class="ui-avatar-group__overflow">
      +{{ overflow }}
    </span>
  </div>
</template>

<style scoped>
.ui-avatar-group {
  display: flex;
  align-items: center;
}

/* Figma gap: -8px → 负 margin 重叠 */
.ui-avatar-group__item + .ui-avatar-group__item,
.ui-avatar-group__overflow {
  margin-left: calc(-1 * var(--size-avatar-overlap));
}

.ui-avatar-group__circle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-full);
  background-color: var(--surface-raised);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-weight: var(--weight-medium);
}

.ui-avatar-group--sm .ui-avatar-group__circle {
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  font-size: var(--font-size-nano);
}

.ui-avatar-group--md .ui-avatar-group__circle {
  width: var(--size-control-md);
  height: var(--size-control-md);
  font-size: var(--font-size-nano);
}

.ui-avatar-group--lg .ui-avatar-group__circle {
  width: var(--size-control-lg);
  height: var(--size-control-lg);
  font-size: var(--font-size-caption);
}

.ui-avatar-group__overflow {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-full);
  background-color: var(--surface-base);
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-nano);
}

.ui-avatar-group--sm .ui-avatar-group__overflow {
  width: var(--size-control-sm);
  height: var(--size-control-sm);
}

.ui-avatar-group--md .ui-avatar-group__overflow {
  width: var(--size-control-md);
  height: var(--size-control-md);
}

.ui-avatar-group--lg .ui-avatar-group__overflow {
  width: var(--size-control-lg);
  height: var(--size-control-lg);
}
</style>
