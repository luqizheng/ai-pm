<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { setLocale, type SupportedLocale } from "@/lib/i18n";

/**
 * 语言快捷开关（顶栏铃铛旁，与主题开关同族）。
 *
 * 单击在中文/英文间往返：按钮显示**目标**语言的短标签
 * （中文界面显示 EN、英文界面显示 中），aria-label 承诺切换后的语言。
 * 状态经 lib/i18n 的 setLocale（与设置偏好页同源，双向互同步），
 * 持久化键 aipm-locale。
 */
const { locale, t } = useI18n();

const target = computed<SupportedLocale>(() =>
  locale.value === "zh-CN" ? "en" : "zh-CN"
);

/** 按钮短标签 = 目标语言原生短码（中 / EN），文案入 yaml 过硬编码门禁 */
const targetLabel = computed(() =>
  target.value === "zh-CN"
    ? t("shell.locale.shortZh")
    : t("shell.locale.shortEn")
);

/** aria-label 指向点击后的目标语言（行为承诺，非当前状态） */
const ariaLabel = computed(() =>
  t("shell.locale.switchTo", { language: t(`shell.locale.${target.value}`) })
);

function onToggle() {
  setLocale(target.value);
}
</script>

<template>
  <button
    type="button"
    class="locale-toggle"
    :aria-label="ariaLabel"
    @click="onToggle"
  >
    {{ targetLabel }}
  </button>
</template>

<style scoped>
/* 与 NotificationBell / ThemeToggle 同规格（36px 高图标位，顶栏同色域） */
.locale-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: var(--size-icon-button);
  height: var(--size-icon-button);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: 0.02em;
  cursor: pointer;
  background: var(--ws-sidebar);
  border: none;
  border-radius: var(--radius-sm);
}

.locale-toggle:hover {
  color: var(--ws-text-primary);
  background: var(--ws-elevated);
}

.locale-toggle:focus-visible {
  outline: none;
  border: var(--border-1) solid var(--ws-accent);
  box-shadow: inset 0 0 0 1px var(--ws-accent);
}
</style>
