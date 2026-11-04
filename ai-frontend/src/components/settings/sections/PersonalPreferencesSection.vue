<script setup lang="ts">
import { ref, onScopeDispose } from "vue";
import { useI18n } from "vue-i18n";

import SettingsFormRow from "../SettingsFormRow.vue";
import SettingsFormSection from "../SettingsFormSection.vue";

import Radio from "@/components/ui/radio/Radio.vue";
import Select from "@/components/ui/select/Select.vue";
import { SUPPORTED_LOCALES, setLocale } from "@/lib/i18n";
import type { SupportedLocale } from "@/lib/i18n";
import { applyTheme, currentTheme } from "@/lib/theme";

/**
 * 个人偏好节内容（共享）：主题三态 + 语言（真实 setLocale）+
 * 时区 + 键盘快捷键（只读表，§6.8）。
 * 设计目录 /design/settings/personal/preferences 与应用页
 * /settings/personal/preferences 共用同一份表单体（§4A Preferences）。
 *
 * 主题语义由 applyToApp 决定：
 * - true（应用页）：写 html data-theme + 持久化 aipm.theme
 *   （经 lib/theme.ts，与 UserMenu 外观面板同链路）；跟随系统实时监听
 *   prefers-color-scheme。持久化契约沿用 index.html 防闪烁脚本
 *   （键 aipm.theme，取值仅 dark/light——system 只存储解析结果）。
 * - false（默认，设计目录演示）：仅作用于本区块 data-theme，
 *   不影响外层壳。
 */
const props = withDefaults(
  defineProps<{
    /** true = 真实切换应用主题（应用页）；false = 本区块演示（设计目录） */
    applyToApp?: boolean;
  }>(),
  { applyToApp: false }
);

const { t, locale } = useI18n();

/** theme: 初始值取当前持久化主题；dark 为默认 */
const theme = ref<"dark" | "light" | "system">(currentTheme());

const themeOptions = [
  { value: "dark", labelKey: "settings.personal.preferences.themeDark" },
  { value: "light", labelKey: "settings.personal.preferences.themeLight" },
  { value: "system", labelKey: "settings.personal.preferences.themeSystem" }
] as const;

/** 系统外观（跟随系统用）；happy-dom 无 matchMedia 时回退深色 */
const systemMedia =
  typeof window !== "undefined" && window.matchMedia
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : undefined;

function applyResolvedTheme(next: "dark" | "light" | "system") {
  if (next === "system") {
    applyTheme(systemMedia?.matches === false ? "light" : "dark");
  } else {
    applyTheme(next);
  }
}

function onThemeChange(next: "dark" | "light" | "system") {
  theme.value = next;
  if (!props.applyToApp) return;
  applyResolvedTheme(next);
}

/** 跟随系统：选中 system 期间实时跟随系统外观切换（帮助文案承诺的行为） */
function onSystemPreferenceChange(event: MediaQueryListEvent) {
  if (props.applyToApp && theme.value === "system") {
    applyTheme(event.matches ? "dark" : "light");
  }
}
systemMedia?.addEventListener?.("change", onSystemPreferenceChange);
onScopeDispose(() => {
  systemMedia?.removeEventListener?.("change", onSystemPreferenceChange);
});

const languageOptions = SUPPORTED_LOCALES.map(value => ({
  value,
  // 语言名按惯例以原生名呈现（不随界面语言切换），文案仍入 yaml 过硬编码门禁
  label:
    value === "zh-CN"
      ? t("settings.personal.preferences.languageNativeZh")
      : t("settings.personal.preferences.languageNativeEn")
}));

const timezoneOptions = [
  { value: "utc8", label: "UTC+08:00 · Shanghai" },
  { value: "utc9", label: "UTC+09:00 · Tokyo" },
  { value: "utc1", label: "UTC+01:00 · Berlin" },
  { value: "utc0", label: "UTC+00:00 · London" },
  { value: "utc-5", label: "UTC-05:00 · New York" }
];

const language = ref<string>(locale.value);
const timezone = ref<string>("utc8");

const shortcuts = [
  {
    actionKey: "settings.personal.preferences.shortcutPalette",
    keys: "⌘ / Ctrl + K"
  },
  {
    actionKey: "settings.personal.preferences.shortcutCommandBar",
    keys: "⌘ / Ctrl + J"
  },
  {
    actionKey: "settings.personal.preferences.shortcutSearch",
    keys: "⌘ / Ctrl + K"
  }
];

function onLanguageChange(value: string) {
  language.value = value;
  setLocale(value as SupportedLocale);
}
</script>

<template>
  <div
    class="preferences-theme"
    :data-theme="!applyToApp && theme === 'light' ? 'light' : undefined"
  >
    <SettingsFormSection
      :title="t('settings.personal.preferences.appearanceTitle')"
    >
      <SettingsFormRow
        :label="t('settings.personal.preferences.theme')"
        :helper="t('settings.personal.preferences.themeHelper')"
      >
        <div
          class="preferences-theme-group"
          role="radiogroup"
          :aria-label="t('settings.personal.preferences.theme')"
        >
          <Radio
            v-for="option in themeOptions"
            :key="option.value"
            :model-value="theme"
            :value="option.value"
            :label="t(option.labelKey)"
            name="settings-theme"
            @update:model-value="onThemeChange"
          />
        </div>
      </SettingsFormRow>
    </SettingsFormSection>

    <SettingsFormSection>
      <SettingsFormRow
        :label="t('settings.personal.preferences.language')"
        :helper="t('settings.personal.preferences.languageHelper')"
      >
        <Select
          :model-value="language"
          :options="languageOptions"
          @update:model-value="onLanguageChange"
        />
      </SettingsFormRow>
      <SettingsFormRow
        :label="t('settings.personal.preferences.timezone')"
        :helper="t('settings.personal.preferences.timezoneHelper')"
      >
        <Select v-model="timezone" :options="timezoneOptions" />
      </SettingsFormRow>
    </SettingsFormSection>
  </div>

  <SettingsFormSection
    :title="t('settings.personal.preferences.shortcutsTitle')"
    :description="t('settings.personal.preferences.shortcutsDesc')"
  >
    <table class="preferences-shortcuts">
      <thead>
        <tr>
          <th>{{ t("settings.personal.preferences.shortcutAction") }}</th>
          <th>{{ t("settings.personal.preferences.shortcutKeys") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="shortcut in shortcuts" :key="shortcut.actionKey">
          <td>{{ t(shortcut.actionKey) }}</td>
          <td>
            <kbd>{{ shortcut.keys }}</kbd>
          </td>
        </tr>
      </tbody>
    </table>
  </SettingsFormSection>
</template>

<style scoped>
/* 主题演示域：radio 切换仅作用于本区块（token 级联），不影响外层壳 */
.preferences-theme {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.preferences-theme-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.preferences-shortcuts {
  width: 100%;
  border-collapse: collapse;
}

.preferences-shortcuts th {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  font-size: var(--ws-font-size-micro);
  font-weight: var(--weight-medium);
  color: var(--ws-text-secondary);
  text-align: left;
}

.preferences-shortcuts td {
  padding: var(--space-2) var(--space-3);
  border-bottom: var(--border-1) solid var(--ws-border);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-primary);
}

.preferences-shortcuts td:last-child {
  text-align: right;
}

.preferences-shortcuts kbd {
  padding: 0 var(--space-1);
  background-color: var(--surface-sunken);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}
</style>
