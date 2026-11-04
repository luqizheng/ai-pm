<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  PERSONAL_NAV,
  SettingsFormRow,
  SettingsFormSection
} from "@/components/settings";

import Radio from "@/components/ui/radio/Radio.vue";
import Select from "@/components/ui/select/Select.vue";

import { DESIGN_PATHS } from "@/router/design";
import { SUPPORTED_LOCALES, setLocale } from "@/lib/i18n";
import type { SupportedLocale } from "@/lib/i18n";
import SettingsFrame from "./SettingsFrame.vue";

/**
 * 帧 05 — 个人偏好设置（§4A Preferences）。
 * 主题三态（真实切换 data-theme 演示）+ 语言（真实 setLocale）+
 * 时区 + 键盘快捷键（只读表，§6.8）。
 */
const { t, locale } = useI18n();

/** theme: dark 为默认；system 跟随系统（演示实时作用于本页根元素） */
const theme = ref<"dark" | "light" | "system">("dark");

const themeOptions = [
  { value: "dark", labelKey: "settings.personal.preferences.themeDark" },
  { value: "light", labelKey: "settings.personal.preferences.themeLight" },
  { value: "system", labelKey: "settings.personal.preferences.themeSystem" }
] as const;

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

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.personal.landing.title"),
    to: DESIGN_PATHS.settings.personal
  },
  { label: t("settings.nav.preferences") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.preferences')"
    :scope-label="t('settings.personal.landing.title')"
    :entries="PERSONAL_NAV"
  >
    <div
      class="preferences-theme"
      :data-theme="theme === 'light' ? 'light' : undefined"
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
              v-model="theme"
              :value="option.value"
              :label="t(option.labelKey)"
              name="settings-theme"
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
  </SettingsFrame>
</template>

<style scoped>
/* 主题演示域：radio 切换仅作用于本区块（token 级联），不影响目录壳 */
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
