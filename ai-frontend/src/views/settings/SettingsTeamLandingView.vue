<script setup lang="ts">
import { useI18n } from "vue-i18n";

import SettingsAppFrame from "./SettingsAppFrame.vue";
import { TEAM_APP_NAV, TEAM_ICONS } from "./nav";

/**
 * 团队设置落地页（团队域独立页面，2026-11-11 用户裁决）。
 *
 * 成员、邀请、AI 配置已迁走；仅 General 保留。入口 = 顶栏账户菜单
 * 「团队设置」。
 */
const { t } = useI18n();

const categoryDescKeys: Record<string, string> = {
  general: "settings.team.landing.generalDesc"
};

const breadcrumb = [{ label: t("settings.team.landing.title") }];
</script>

<template>
  <!-- 壳内页面自带留白（.workspace-main-scroll 无 padding，同个人落地页口径） -->
  <div class="settings-shell-pad">
    <SettingsAppFrame
      :breadcrumb="breadcrumb"
      :title="t('settings.team.landing.title')"
      :description="t('settings.team.landing.desc')"
      :scope-label="t('settings.team.landing.title')"
      :entries="TEAM_APP_NAV"
      :icons="TEAM_ICONS"
    >
      <section class="team-landing__grid">
        <RouterLink
          v-for="entry in TEAM_APP_NAV"
          :key="entry.id"
          class="team-landing__card"
          :to="entry.to"
        >
          <component
            :is="TEAM_ICONS[entry.id]"
            class="team-landing__icon"
            aria-hidden="true"
          />
          <h2 class="team-landing__card-title">
            {{ t(`settings.nav.${entry.id}`) }}
          </h2>
          <p class="team-landing__card-desc">
            {{ t(categoryDescKeys[entry.id]) }}
          </p>
        </RouterLink>
      </section>
    </SettingsAppFrame>
  </div>
</template>

<style scoped>
.settings-shell-pad {
  padding: var(--space-5) var(--space-8);
}

.team-landing__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.team-landing__card {
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

.team-landing__card:hover {
  border-color: var(--ws-accent);
}

.team-landing__icon {
  width: var(--size-icon-md);
  height: var(--size-icon-md);
  color: var(--ws-accent);
}

.team-landing__card-title {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.team-landing__card-desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.team-landing__card:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

@media (prefers-reduced-motion: reduce) {
  .team-landing__card {
    transition: none;
  }
}

@media (max-width: 880px) {
  .team-landing__grid {
    grid-template-columns: 1fr;
  }
}
</style>
