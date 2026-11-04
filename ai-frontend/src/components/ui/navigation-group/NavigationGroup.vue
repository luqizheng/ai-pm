<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Navigation/NavigationGroup" node=13:33391 date=2026-11-04 -->
<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * NavigationGroup —— Figma DS/Navigation/NavigationGroup（13:33391）。
 * 组标题行（36px，mono 大写小字号标题 + chevron + 计数徽标）+ 子项插槽（左缩进）。
 * Expanded / Collapsed 两态；受控（v-model:collapsed）与非受控（defaultCollapsed）兼容。
 */

interface Props {
  /** 组标题（Figma 为全大写展示，如 PROJECT） */
  title: string;
  collapsed?: boolean;
  defaultCollapsed?: boolean;
  /** 标题右侧计数徽标 */
  badge?: number;
}

const props = withDefaults(defineProps<Props>(), {
  collapsed: undefined,
  defaultCollapsed: false,
  badge: undefined
});

const emit = defineEmits<{
  "update:collapsed": [value: boolean];
  toggle: [value: boolean];
}>();

/** 非受控折叠态（collapsed prop 未传时生效） */
const innerCollapsed = ref(props.defaultCollapsed);

const isCollapsed = computed(() =>
  props.collapsed === undefined ? innerCollapsed.value : props.collapsed
);

function onToggle() {
  const next = !isCollapsed.value;
  innerCollapsed.value = next;
  emit("update:collapsed", next);
  emit("toggle", next);
}
</script>

<template>
  <section :class="cn('ui-nav-group', { 'is-collapsed': isCollapsed })">
    <button
      type="button"
      class="ui-nav-group__toggle"
      :aria-expanded="isCollapsed ? 'false' : 'true'"
      @click="onToggle"
    >
      <span class="ui-nav-group__title">{{ title }}</span>
      <span v-if="badge !== undefined" class="ui-nav-group__badge">{{
        badge
      }}</span>
      <ChevronDown
        v-if="!isCollapsed"
        class="ui-nav-group__chevron"
        aria-hidden="true"
      />
      <ChevronRight v-else class="ui-nav-group__chevron" aria-hidden="true" />
    </button>

    <div v-if="!isCollapsed" class="ui-nav-group__items">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.ui-nav-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.ui-nav-group__toggle {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: var(--size-nav-item);
  padding: 0 var(--space-3);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  cursor: pointer;
  transition: background-color var(--duration-fast) var(--ease-decelerate);
}

.ui-nav-group__toggle:hover {
  background-color: var(--surface-raised);
}

.ui-nav-group__toggle:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

/* 组标题：mono 小字号大写 */
.ui-nav-group__title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  letter-spacing: var(--tracking-caption);
  text-transform: uppercase;
  color: var(--text-secondary);
}

.ui-nav-group__badge {
  display: inline-flex;
  align-items: center;
  min-width: var(--size-nav-badge);
  padding: 0 var(--space-1);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  font-family: var(--font-ui);
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
}

.ui-nav-group__chevron {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  flex: none;
  color: var(--text-secondary);
}

/* 子项左缩进（Figma Destinations padding-left 8px） */
.ui-nav-group__items {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding-left: var(--size-nav-indent);
}
</style>
