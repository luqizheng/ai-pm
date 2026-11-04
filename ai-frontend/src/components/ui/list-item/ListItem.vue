<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Data/ListItem" node=13:34428 date=2026-11-04 -->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { cn } from "@/lib/cn";

/**
 * ListItem —— Figma DS/Data/ListItem（13:34428）。
 * row 8/12 padding gap 8 radius 8；标题 13px + 描述 11px 次文字 + trailing 插槽；
 * selected 底 accent 着色面；hover 抬升面；disabled 0.4。
 */

export type ListItemSize = "sm" | "md" | "lg";

interface Props {
  title: string;
  description?: string;
  icon?: Component;
  active?: boolean;
  selected?: boolean;
  disabled?: boolean;
  size?: ListItemSize;
}

const props = withDefaults(defineProps<Props>(), {
  description: undefined,
  icon: undefined,
  active: false,
  selected: false,
  disabled: false,
  size: "md"
});

const emit = defineEmits<{ click: [event: MouseEvent] }>();

const hasDescription = computed(() => Boolean(props.description));
</script>

<template>
  <button
    type="button"
    :class="
      cn('ui-list-item', `ui-list-item--${size}`, {
        'is-selected': selected,
        'is-active': active,
        'is-disabled': disabled
      })
    "
    :aria-selected="selected ? 'true' : undefined"
    :disabled="disabled"
    @click="emit('click', $event)"
  >
    <component
      :is="icon"
      v-if="icon"
      class="ui-list-item__icon"
      aria-hidden="true"
    />
    <span class="ui-list-item__content">
      <span class="ui-list-item__title">{{ title }}</span>
      <span v-if="hasDescription" class="ui-list-item__description">{{
        description
      }}</span>
    </span>
    <span class="ui-list-item__trailing">
      <slot name="trailing" />
    </span>
  </button>
</template>

<style scoped>
.ui-list-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  text-align: left;
  cursor: pointer;
  transition: background-color var(--duration-fast) var(--ease-decelerate);
}

.ui-list-item--sm {
  padding: var(--space-1) var(--space-3);
}

.ui-list-item--lg {
  min-height: var(--size-row);
}

.ui-list-item:hover:not(.is-disabled) {
  background-color: var(--surface-raised);
}

.ui-list-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-list-item.is-selected {
  background-color: var(--accent-surface);
}

.ui-list-item.is-disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}

.ui-list-item__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

.ui-list-item__content {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.ui-list-item__title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ui-list-item__description {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.ui-list-item__trailing {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
  color: var(--text-secondary);
}
</style>
