<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Data/NotificationItem" node=13:34472 date=2026-11-04 -->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { cn } from "@/lib/cn";

/**
 * NotificationItem —— Figma DS/Data/NotificationItem（13:34472）。
 * 未读行底 accent 着色面 + accent 状态点；标题 13px + 正文 11px + 时间戳。
 */
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";

interface Props {
  title: string;
  body?: string;
  unread?: boolean;
  timestamp?: string;
  icon?: Component;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  body: undefined,
  unread: false,
  timestamp: "",
  icon: undefined,
  disabled: false
});

const emit = defineEmits<{ click: [event: MouseEvent] }>();

const hasBody = computed(() => Boolean(props.body));
const hasTimestamp = computed(() => props.timestamp.length > 0);
</script>

<template>
  <button
    type="button"
    :class="
      cn('ui-notification', { 'is-unread': unread, 'is-disabled': disabled })
    "
    :disabled="disabled"
    @click="emit('click', $event)"
  >
    <StatusIndicator
      v-if="unread"
      status="accent"
      size="sm"
      class="ui-notification__dot"
    />
    <component
      :is="icon"
      v-else-if="icon"
      class="ui-notification__icon"
      aria-hidden="true"
    />
    <span class="ui-notification__content">
      <span class="ui-notification__title">{{ title }}</span>
      <span v-if="hasBody" class="ui-notification__body">{{ body }}</span>
    </span>
    <time v-if="hasTimestamp" class="ui-notification__time">{{
      timestamp
    }}</time>
  </button>
</template>

<style scoped>
.ui-notification {
  display: flex;
  align-items: flex-start;
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

.ui-notification:hover:not(.is-disabled) {
  background-color: var(--surface-raised);
}

.ui-notification:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-notification.is-unread {
  background-color: var(--accent-surface);
}

.ui-notification.is-disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}

.ui-notification__dot {
  flex: none;
  margin-top: var(--space-2);
}

.ui-notification__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  margin-top: var(--space-2);
  color: var(--text-secondary);
}

.ui-notification__content {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.ui-notification__title {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.ui-notification__body {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.ui-notification__time {
  flex: none;
  font-family: var(--font-ui);
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
}
</style>
