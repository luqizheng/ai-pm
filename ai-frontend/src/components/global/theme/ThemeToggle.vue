<script setup lang="ts">
import { computed } from "vue";
import { Moon, Sun } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import { useTheme } from "@/lib/theme";

/**
 * 主题快捷开关（顶栏铃铛旁；原 UserMenu 外观面板上移收编）。
 *
 * 单击在深/浅间往返：当前深色 → 显示 Sun（点击转浅色），
 * 当前浅色 → 显示 Moon（点击转深色）。
 * 状态经 lib/theme 的共享响应式（与设置偏好页同源，双向互同步），
 * 持久化键 aipm.theme 与 index.html 防闪烁脚本同值。
 */
const { theme, toggleTheme } = useTheme();

const { t } = useI18n();

const isDark = computed(() => theme.value === "dark");

/** aria-label 指向点击后的目标模式（行为承诺，非当前状态） */
const ariaLabel = computed(() =>
  isDark.value ? t("global.theme.toLight") : t("global.theme.toDark")
);
</script>

<template>
  <button
    type="button"
    class="theme-toggle"
    :aria-label="ariaLabel"
    @click="toggleTheme()"
  >
    <Sun v-if="isDark" :size="16" aria-hidden="true" />
    <Moon v-else :size="16" aria-hidden="true" />
  </button>
</template>

<style scoped>
/* 与 NotificationBell 同规格（36×36 图标钮，顶栏同色域） */
.theme-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-button);
  height: var(--size-icon-button);
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: var(--ws-sidebar);
  border: none;
  border-radius: var(--radius-sm);
}

.theme-toggle:hover {
  color: var(--ws-text-primary);
  background: var(--ws-elevated);
}

.theme-toggle:focus-visible {
  outline: none;
  border: var(--border-1) solid var(--ws-accent);
  box-shadow: inset 0 0 0 1px var(--ws-accent);
}
</style>
