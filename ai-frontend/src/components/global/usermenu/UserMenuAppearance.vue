<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/UserMenu/Appearance" node=33:87794 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import UserMenuItem from "./UserMenuItem.vue";
import { applyTheme, currentTheme, type Theme } from "@/lib/theme";

/**
 * 外观面板（Figma Global/UserMenu/Appearance 33:87794）。
 *
 * 标题 + Dark/Light 两项（选中 = MenuItem Selected：accent 面 + check）+
 * 13px 说明行；切换即写 html data-theme + 持久化 aipm.theme
 * （与 index.html 防闪烁脚本同键同值）。
 */
const model = defineModel<Theme>({ default: undefined });

const { t } = useI18n();

const theme = computed<Theme>(() => model.value ?? currentTheme());

const note = computed(() =>
  t("global.usermenu.themeNote", {
    theme:
      theme.value === "light"
        ? t("global.usermenu.themeLight")
        : t("global.usermenu.themeDark")
  })
);

function select(next: Theme) {
  model.value = next;
  applyTheme(next);
}
</script>

<template>
  <div class="user-menu-appearance">
    <p class="user-menu-appearance__title">
      {{ t("global.usermenu.appearance") }}
    </p>
    <UserMenuItem
      :label="t('global.usermenu.themeDark')"
      :variant="theme === 'dark' ? 'selected' : 'default'"
      @click="select('dark')"
    />
    <UserMenuItem
      :label="t('global.usermenu.themeLight')"
      :variant="theme === 'light' ? 'selected' : 'default'"
      @click="select('light')"
    />
    <p class="user-menu-appearance__note">{{ note }}</p>
  </div>
</template>

<style scoped>
.user-menu-appearance {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.user-menu-appearance__title {
  margin: 0;
  padding: 0 var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.user-menu-appearance__note {
  margin: 0;
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}
</style>
