<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Navigation/Tab" node=13:33477 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { TabsContent, TabsList, TabsRoot, TabsTrigger } from "reka-ui";
import { cn } from "@/lib/cn";
import type { TabItem } from "./types";

interface Props {
  /** 当前页签值（受控；不传时组件内部自管理，默认第一个） */
  modelValue?: string;
  /** 页签列表 */
  tabs?: TabItem[];
  /** 页签栏的无障碍名称 */
  ariaLabel?: string;
  /** 排列方向 */
  orientation?: "horizontal" | "vertical";
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: undefined,
  tabs: () => [],
  ariaLabel: undefined,
  orientation: "horizontal"
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

/** 未指定 modelValue 时默认选中第一个页签 */
const fallbackValue = computed(() => props.tabs[0]?.value);

function onUpdate(value: string | number) {
  emit("update:modelValue", String(value));
}
</script>

<template>
  <TabsRoot
    class="ui-tabs"
    :model-value="modelValue"
    :default-value="modelValue ?? fallbackValue"
    :orientation="orientation"
    @update:model-value="onUpdate"
  >
    <TabsList class="ui-tabs__list" :aria-label="ariaLabel">
      <TabsTrigger
        v-for="tab in tabs"
        :key="tab.value"
        class="ui-tabs__trigger"
        :class="cn({ 'is-disabled': tab.disabled })"
        :value="tab.value"
        :disabled="tab.disabled"
      >
        <slot name="tab" :tab="tab">{{ tab.label }}</slot>
      </TabsTrigger>
    </TabsList>

    <TabsContent
      v-for="tab in tabs"
      :key="tab.value"
      class="ui-tabs__panel"
      :value="tab.value"
    >
      <slot name="panel" :tab="tab" :value="tab.value" />
    </TabsContent>
  </TabsRoot>
</template>

<style scoped>
/* Figma DS/Navigation/Tab（13:33477）：
 * 透明底行（无整条底线）/ 项 padding 0 12px / Hover=surface 底文字不变 /
 * Active=透明底 + 主文字 + 仅底部 2px accent 下划线（strokeWeight 0 0 2px）/
 * Focus=2px accent 四边 / Disabled=opacity 0.40。 */
.ui-tabs__list {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.ui-tabs__trigger {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-secondary);
  white-space: nowrap;
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease-decelerate),
    background-color var(--duration-fast) var(--ease-decelerate);
}

/* Hover：surface 底 + 文字保持次文字（Figma 口径） */
.ui-tabs__trigger:hover:not(:disabled) {
  background-color: var(--surface-base);
  color: var(--text-secondary);
}

.ui-tabs__trigger:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
  border-radius: var(--radius-xs);
}

/* Active：透明底 + 主文字 + 底缘 2px accent 下划线（仅此一处强调） */
.ui-tabs__trigger[data-state="active"] {
  background: transparent;
  color: var(--text-primary);
}

/* 指示条 2px（Figma strokeWeight "0px 0px 2px"） */
.ui-tabs__trigger[data-state="active"]::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--accent);
  content: "";
}

.ui-tabs__trigger:disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}

/* 面板：切换时 240ms 淡入 */
.ui-tabs__panel {
  padding-top: var(--space-4);
  animation: ui-tabs-panel-enter var(--duration-medium) var(--ease-standard);
}

.ui-tabs__panel:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  border-radius: var(--radius-xs);
}

@keyframes ui-tabs-panel-enter {
  from {
    opacity: 0;
  }

  to {
    opacity: 1;
  }
}

/* 减少动效 — Phase 2 §5.4 #1：仅 opacity 80ms */
@media (prefers-reduced-motion: reduce) {
  .ui-tabs__panel {
    animation: ui-tabs-panel-enter var(--duration-instant) linear;
  }
}
</style>
