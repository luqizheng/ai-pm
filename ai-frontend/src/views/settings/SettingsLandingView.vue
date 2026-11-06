<script setup lang="ts">
import { useI18n } from "vue-i18n";

import SettingsPageLayout from "@/components/settings/SettingsPageLayout.vue";
import SettingsSidebar from "@/components/settings/SettingsSidebar.vue";

import {
  PERSONAL_APP_NAV,
  PERSONAL_ICONS,
  TEAM_APP_NAV,
  TEAM_ICONS
} from "./nav";

/**
 * 应用设置落地页（个人域，§4A）。
 * 五个类目卡（资料/安全/偏好/通知/AI 偏好），个人域恒为本人授权；
 * 入口 = 顶栏账户菜单（UserMenu「账户设置」），结构与设计目录
 * 落地页一致，卡片指向应用路径 /settings/personal/*。
 */
const { t } = useI18n();

const categoryDescKeys: Record<string, string> = {
  profile: "settings.personal.categories.profile",
  security: "settings.personal.categories.security",
  preferences: "settings.personal.categories.preferences",
  notifications: "settings.personal.categories.notifications",
  ai: "settings.personal.categories.ai"
};

const breadcrumb = [{ label: t("settings.personal.landing.title") }];
</script>

<template>
  <!-- 壳内页面自带留白（.workspace-main-scroll 无 padding，同 PlaceholderView 口径） -->
  <div class="settings-shell-pad">
    <SettingsPageLayout
      :breadcrumb="breadcrumb"
      :title="t('settings.personal.landing.title')"
      :description="t('settings.personal.landing.desc')"
      :icons="PERSONAL_ICONS"
    >
      <template #sidebar>
        <SettingsSidebar
          :scope-label="t('settings.personal.landing.title')"
          :entries="PERSONAL_APP_NAV"
          :icons="PERSONAL_ICONS"
        />
      </template>

      <section class="personal-landing__hint">
        <p class="personal-landing__hint-text">
          {{ t("settings.architecture.scope.personal.entry") }}
        </p>
      </section>

      <section class="personal-landing__grid">
        <RouterLink
          v-for="entry in PERSONAL_APP_NAV"
          :key="entry.id"
          class="personal-landing__card"
          :to="entry.to"
        >
          <component
            :is="PERSONAL_ICONS[entry.id]"
            class="personal-landing__icon"
            aria-hidden="true"
          />
          <h2 class="personal-landing__card-title">
            {{ t(`settings.nav.${entry.id}`) }}
          </h2>
          <p class="personal-landing__card-desc">
            {{ t(categoryDescKeys[entry.id]) }}
          </p>
          <span v-if="entry.proposed" class="personal-landing__proposed">{{
            t("settings.common.proposed")
          }}</span>
        </RouterLink>
      </section>

      <!-- 团队设置区（0041 起）：Team≡tenant 的团队级配置入口 -->
      <h2 class="personal-landing__scope-title">
        {{ t("settings.team.landing.title") }}
      </h2>
      <section class="personal-landing__grid">
        <RouterLink
          v-for="entry in TEAM_APP_NAV"
          :key="entry.id"
          class="personal-landing__card"
          :to="entry.to"
        >
          <component
            :is="TEAM_ICONS[entry.id]"
            class="personal-landing__icon"
            aria-hidden="true"
          />
          <h2 class="personal-landing__card-title">
            {{ t(`settings.nav.${entry.id}`) }}
          </h2>
          <p class="personal-landing__card-desc">
            {{ t("settings.team.landing.aiDesc") }}
          </p>
        </RouterLink>
      </section>
    </SettingsPageLayout>
  </div>
</template>

<style scoped>
.settings-shell-pad {
  padding: var(--space-5) var(--space-8);
}

.personal-landing__scope-title {
  margin: var(--space-6) 0 var(--space-3);
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.personal-landing__hint {
  margin: 0 0 var(--space-4);
}

.personal-landing__hint-text {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-accent);
}

.personal-landing__grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

.personal-landing__card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  text-decoration: none;
  transition: border-color var(--duration-fast) var(--ease-standard);
}

.personal-landing__card:hover {
  border-color: var(--ws-accent);
}

.personal-landing__icon {
  width: var(--size-icon-md);
  height: var(--size-icon-md);
  color: var(--ws-accent);
}

.personal-landing__card-title {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.personal-landing__card-desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.personal-landing__proposed {
  position: absolute;
  top: var(--space-3);
  right: var(--space-3);
  padding: 0 var(--space-1);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.personal-landing__card:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

@media (prefers-reduced-motion: reduce) {
  .personal-landing__card {
    transition: none;
  }
}
</style>
