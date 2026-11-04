<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Building2, FolderCog, UserRound } from "lucide-vue-next";

import { DESIGN_PATHS } from "@/router/design";
import DesignLayout from "../DesignLayout.vue";

/**
 * 帧 01 — Settings 架构总览（§4/§9/§10）。
 * 三域卡片（入口+边界）+ 框架原则 + 帧索引。
 */
const { t } = useI18n();

const scopes = [
  {
    icon: UserRound,
    titleKey: "settings.architecture.scope.personal.title",
    entryKey: "settings.architecture.scope.personal.entry",
    descKey: "settings.architecture.scope.personal.desc",
    to: DESIGN_PATHS.settings.personal
  },
  {
    icon: Building2,
    titleKey: "settings.architecture.scope.tenant.title",
    entryKey: "settings.architecture.scope.tenant.entry",
    descKey: "settings.architecture.scope.tenant.desc",
    to: DESIGN_PATHS.settings.tenant
  },
  {
    icon: FolderCog,
    titleKey: "settings.architecture.scope.project.title",
    entryKey: "settings.architecture.scope.project.entry",
    descKey: "settings.architecture.scope.project.desc",
    to: DESIGN_PATHS.settings.project
  }
] as const;

const principleKeys = [
  "settings.architecture.principles.p1",
  "settings.architecture.principles.p2",
  "settings.architecture.principles.p3",
  "settings.architecture.principles.p4",
  "settings.architecture.principles.p5"
] as const;

const frames = [
  {
    no: "02",
    labelKey: "settings.personal.landing.title",
    to: DESIGN_PATHS.settings.personal
  },
  {
    no: "03",
    labelKey: "settings.nav.profile",
    to: DESIGN_PATHS.settings.personalProfile
  },
  {
    no: "04",
    labelKey: "settings.nav.security",
    to: DESIGN_PATHS.settings.personalSecurity
  },
  {
    no: "05",
    labelKey: "settings.nav.preferences",
    to: DESIGN_PATHS.settings.personalPreferences
  },
  {
    no: "06",
    labelKey: "settings.nav.notifications",
    to: DESIGN_PATHS.settings.personalNotifications
  },
  {
    no: "07",
    labelKey: "settings.nav.ai",
    to: DESIGN_PATHS.settings.personalAi
  },
  {
    no: "08",
    labelKey: "settings.tenant.landing.title",
    to: DESIGN_PATHS.settings.tenant
  },
  {
    no: "09",
    labelKey: "settings.nav.tenantGeneral",
    to: DESIGN_PATHS.settings.tenantGeneral
  },
  {
    no: "10",
    labelKey: "settings.nav.members",
    to: DESIGN_PATHS.settings.tenantMembers
  },
  {
    no: "11",
    labelKey: "settings.nav.integrations",
    to: DESIGN_PATHS.settings.tenantIntegrations
  },
  {
    no: "12",
    labelKey: "settings.project.landing.title",
    to: DESIGN_PATHS.settings.project
  },
  {
    no: "13",
    labelKey: "settings.nav.projectGeneral",
    to: DESIGN_PATHS.settings.projectGeneral
  },
  {
    no: "14",
    labelKey: "settings.nav.projectMembers",
    to: DESIGN_PATHS.settings.projectMembers
  },
  {
    no: "15",
    labelKey: "settings.nav.repositories",
    to: DESIGN_PATHS.settings.projectRepositories
  },
  {
    no: "16",
    labelKey: "settings.nav.development",
    to: DESIGN_PATHS.settings.projectDevelopment
  },
  {
    no: "17",
    labelKey: "settings.nav.projectAi",
    to: DESIGN_PATHS.settings.projectAi
  },
  {
    no: "18",
    labelKey: "settings.states.title",
    to: DESIGN_PATHS.settings.states
  },
  {
    no: "19",
    labelKey: "settings.components.title",
    to: DESIGN_PATHS.settings.components
  }
] as const;
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="settings-arch__title">
        {{ t("settings.architecture.title") }}
      </h1>
      <p class="settings-arch__desc">{{ t("settings.architecture.desc") }}</p>
    </template>

    <section class="settings-arch__block">
      <h2 class="settings-arch__heading">
        {{ t("settings.architecture.scopesTitle") }}
      </h2>
      <p class="settings-arch__sub">
        {{ t("settings.architecture.scopesDesc") }}
      </p>
      <div class="settings-arch__scopes">
        <RouterLink
          v-for="scope in scopes"
          :key="scope.titleKey"
          class="settings-arch__scope"
          :to="scope.to"
        >
          <component
            :is="scope.icon"
            class="settings-arch__scope-icon"
            aria-hidden="true"
          />
          <h3 class="settings-arch__scope-title">{{ t(scope.titleKey) }}</h3>
          <p class="settings-arch__scope-entry">{{ t(scope.entryKey) }}</p>
          <p class="settings-arch__scope-desc">{{ t(scope.descKey) }}</p>
        </RouterLink>
      </div>
    </section>

    <section class="settings-arch__block">
      <h2 class="settings-arch__heading">
        {{ t("settings.architecture.principlesTitle") }}
      </h2>
      <ul class="settings-arch__principles">
        <li v-for="key in principleKeys" :key="key">{{ t(key) }}</li>
      </ul>
      <p class="settings-arch__legend">
        <span class="settings-arch__legend-badge">{{
          t("settings.common.proposed")
        }}</span>
        {{ t("settings.architecture.legendProposed") }}
      </p>
    </section>

    <section class="settings-arch__block">
      <h2 class="settings-arch__heading">
        {{ t("settings.architecture.framesTitle") }}
      </h2>
      <div class="settings-arch__frames">
        <RouterLink
          v-for="frame in frames"
          :key="frame.no"
          class="settings-arch__frame"
          :to="frame.to"
        >
          <span class="settings-arch__frame-no">{{ frame.no }}</span>
          <span class="settings-arch__frame-label">{{
            t(frame.labelKey)
          }}</span>
        </RouterLink>
      </div>
    </section>
  </DesignLayout>
</template>

<style scoped>
.settings-arch__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.settings-arch__desc {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.settings-arch__block {
  margin: 0 0 var(--space-8);
}

.settings-arch__heading {
  margin: 0 0 var(--space-1);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.settings-arch__sub {
  margin: 0 0 var(--space-4);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.settings-arch__scopes {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

.settings-arch__scope {
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

.settings-arch__scope:hover {
  border-color: var(--ws-accent);
}

.settings-arch__scope-icon {
  width: var(--size-icon-md);
  height: var(--size-icon-md);
  color: var(--ws-accent);
}

.settings-arch__scope-title {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.settings-arch__scope-entry {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-accent);
}

.settings-arch__scope-desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.settings-arch__principles {
  margin: 0;
  padding-inline-start: var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.settings-arch__legend {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: var(--space-3) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.settings-arch__legend-badge {
  flex: none;
  padding: 0 var(--space-1);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.settings-arch__frames {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: var(--space-2);
}

.settings-arch__frame {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  text-decoration: none;
  transition: border-color var(--duration-fast) var(--ease-standard);
}

.settings-arch__frame:hover {
  border-color: var(--ws-accent);
}

.settings-arch__frame-no {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-accent);
}

.settings-arch__frame-label {
  overflow: hidden;
  font-size: var(--ws-font-size-micro);
  color: var(--ws-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.settings-arch__scope:focus-visible,
.settings-arch__frame:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

@media (prefers-reduced-motion: reduce) {
  .settings-arch__scope,
  .settings-arch__frame {
    transition: none;
  }
}
</style>
