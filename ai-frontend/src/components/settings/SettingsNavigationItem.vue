<script setup lang="ts">
import { computed } from "vue";
import type { Component } from "vue";
import { useI18n } from "vue-i18n";

/**
 * SettingsNavigationItem —— 设置侧栏导航项（Settings Framework §7）。
 *
 * 目标可用且未禁用时渲染 RouterLink，否则渲染按钮/占位；
 * Proposed 条目（§9 能力未确认）挂微徽标，不与「已实现」混排。
 */
const props = defineProps<{
  /** 条目文案（已翻译） */
  label: string;
  /** 目标路由（空 = 占位项） */
  to?: string;
  /** 图标组件（lucide） */
  icon?: Component;
  /** 当前激活态 */
  active?: boolean;
  /** Proposed 徽标（能力未确认） */
  proposed?: boolean;
  /** 无权访问（置灰，仍可点击进入 denied 说明页） */
  disabled?: boolean;
  /** 右侧帧号标注（设计目录索引用） */
  frame?: string;
}>();

const { t } = useI18n();

const proposedLabel = computed(() => t("settings.common.proposed"));
</script>

<template>
  <RouterLink
    v-if="to && !disabled"
    :to="to"
    class="settings-nav-item"
    :class="{ 'is-active': active }"
  >
    <component
      :is="icon"
      v-if="icon"
      class="settings-nav-item__icon"
      aria-hidden="true"
    />
    <span class="settings-nav-item__label">{{ label }}</span>
    <span v-if="proposed" class="settings-nav-item__proposed">{{
      proposedLabel
    }}</span>
    <span v-else-if="frame" class="settings-nav-item__frame">{{ frame }}</span>
  </RouterLink>
  <button
    v-else
    type="button"
    class="settings-nav-item"
    :class="{ 'is-active': active, 'is-disabled': disabled }"
    :disabled="disabled"
  >
    <component
      :is="icon"
      v-if="icon"
      class="settings-nav-item__icon"
      aria-hidden="true"
    />
    <span class="settings-nav-item__label">{{ label }}</span>
    <span v-if="proposed" class="settings-nav-item__proposed">{{
      proposedLabel
    }}</span>
    <span v-else-if="frame" class="settings-nav-item__frame">{{ frame }}</span>
  </button>
</template>

<style scoped>
.settings-nav-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  min-height: var(--size-nav-item);
  padding: 0 var(--space-3);
  border: none;
  border-radius: var(--radius-sm);
  background-color: transparent;
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  text-align: left;
  text-decoration: none;
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard);
}

.settings-nav-item:hover {
  background-color: var(--surface-hover-veil);
  color: var(--ws-text-primary);
}

.settings-nav-item.is-active {
  background-color: var(--accent-surface);
  color: var(--ws-text-primary);
}

.settings-nav-item.is-disabled {
  opacity: var(--opacity-disabled-option);
  cursor: not-allowed;
}

.settings-nav-item__icon {
  flex: none;
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
}

.settings-nav-item__label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.settings-nav-item__proposed {
  flex: none;
  padding: 0 var(--space-1);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.settings-nav-item__frame {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-disabled);
}

.settings-nav-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

@media (prefers-reduced-motion: reduce) {
  .settings-nav-item {
    transition: none;
  }
}
</style>
