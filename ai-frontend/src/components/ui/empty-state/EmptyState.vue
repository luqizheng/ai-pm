<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Feedback/EmptyState" node=13:34322 date=2026-11-04 -->
<script setup lang="ts">
import { computed, type Component } from "vue";
import { PackageOpen } from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * EmptyState —— Figma DS/Feedback/EmptyState（13:34322）。
 * 居中列布局：圆形图标容器（24px 图标）+ 标题（13 SemiBold）+ 描述（11 次文字）
 * + 动作区（主按钮 + 次按钮）。
 * ⚠ Figma 源矛盾备注：规格记录「两个 Action 文案样式与 Create item 不一致」，
 * 本实现按一致按钮口径实现，文案由 props 传入。
 */

export type EmptyStateSize = "sm" | "md" | "lg";

interface Props {
  title: string;
  description?: string;
  icon?: Component;
  size?: EmptyStateSize;
  actionLabel?: string;
  secondaryActionLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  description: undefined,
  icon: undefined,
  size: "md",
  actionLabel: "",
  secondaryActionLabel: ""
});

const emit = defineEmits<{ action: []; secondaryAction: [] }>();

const hasDescription = computed(() => Boolean(props.description));
const hasAction = computed(() => props.actionLabel.length > 0);
const hasSecondaryAction = computed(
  () => props.secondaryActionLabel.length > 0
);
</script>

<template>
  <div :class="cn('ui-empty', `ui-empty--${size}`)" role="status">
    <span class="ui-empty__icon-ring" aria-hidden="true">
      <PackageOpen v-if="!icon" class="ui-empty__icon" />
      <component :is="icon" v-else class="ui-empty__icon" />
    </span>

    <p class="ui-empty__title">{{ title }}</p>
    <p v-if="hasDescription" class="ui-empty__description">{{ description }}</p>

    <div v-if="hasAction || hasSecondaryAction" class="ui-empty__actions">
      <button
        v-if="hasAction"
        type="button"
        class="ui-empty__btn ui-empty__btn--primary"
        @click="emit('action')"
      >
        {{ actionLabel }}
      </button>
      <button
        v-if="hasSecondaryAction"
        type="button"
        class="ui-empty__btn ui-empty__btn--secondary"
        @click="emit('secondaryAction')"
      >
        {{ secondaryActionLabel }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.ui-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-6);
  text-align: center;
}

.ui-empty--sm {
  padding: var(--space-4);
}

.ui-empty--lg {
  padding: var(--space-8);
}

.ui-empty__icon-ring {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-button);
  height: var(--size-icon-button);
  border-radius: var(--radius-full);
  background-color: var(--surface-raised);
  color: var(--text-secondary);
}

.ui-empty--lg .ui-empty__icon-ring {
  width: var(--size-control-lg);
  height: var(--size-control-lg);
}

.ui-empty__icon {
  width: var(--size-empty-icon);
  height: var(--size-empty-icon);
}

.ui-empty__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ui-empty__description {
  margin: 0;
  max-width: 42ch;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.ui-empty__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.ui-empty__btn {
  display: inline-flex;
  align-items: center;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  cursor: pointer;
  transition: background-color var(--duration-fast) var(--ease-decelerate);
}

.ui-empty__btn:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ui-empty__btn--primary {
  background-color: var(--accent);
  color: var(--accent-contrast-text);
}

.ui-empty__btn--primary:hover {
  background-color: var(--accent-hover);
}

.ui-empty__btn--secondary {
  background-color: var(--surface-raised);
  border-color: var(--border-strong);
  color: var(--text-primary);
}

.ui-empty__btn--secondary:hover {
  background-color: var(--ws-hover-veil);
}
</style>
