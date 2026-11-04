<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Bell,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  UserRound
} from "lucide-vue-next";

import {
  PERSONAL_NAV,
  SettingsPageLayout,
  SettingsSidebar
} from "@/components/settings";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 02 — 个人账户设置落地页（§4A）。
 * 五个类目卡（资料/安全/偏好/通知/AI 偏好）；个人域恒为本人授权。
 */
const { t } = useI18n();

const icons = {
  profile: UserRound,
  security: ShieldCheck,
  preferences: SlidersHorizontal,
  notifications: Bell,
  ai: Sparkles
};

const categoryDescKeys: Record<string, string> = {
  profile: "settings.personal.categories.profile",
  security: "settings.personal.categories.security",
  preferences: "settings.personal.categories.preferences",
  notifications: "settings.personal.categories.notifications",
  ai: "settings.personal.categories.ai"
};

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.architecture.title"),
    to: DESIGN_PATHS.settings.architecture
  },
  { label: t("settings.personal.landing.title") }
];
</script>

<template>
  <SettingsPageLayout
    :breadcrumb="breadcrumb"
    :title="t('settings.personal.landing.title')"
    :description="t('settings.personal.landing.desc')"
    :icons="icons"
  >
    <template #sidebar>
      <SettingsSidebar
        :scope-label="t('settings.personal.landing.title')"
        :entries="PERSONAL_NAV"
        :icons="icons"
      />
    </template>

    <section class="personal-landing__intro">
      <p class="personal-landing__hint">
        {{ t("settings.architecture.scope.personal.entry") }}
      </p>
    </section>

    <section class="personal-landing__grid">
      <RouterLink
        v-for="entry in PERSONAL_NAV"
        :key="entry.id"
        class="personal-landing__card"
        :to="entry.to"
      >
        <component
          :is="icons[entry.id]"
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
  </SettingsPageLayout>
</template>

<style scoped>
.personal-landing__hint {
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
