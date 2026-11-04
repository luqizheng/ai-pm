<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Blocks,
  GitBranch,
  Sparkles,
  Terminal,
  UsersRound,
  FolderCog
} from "lucide-vue-next";

import {
  PROJECT_NAV,
  SettingsPageLayout,
  SettingsSidebar,
  useSettingsDemo
} from "@/components/settings";

import Radio from "@/components/ui/radio/Radio.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 12 — 项目设置落地页（§4C）。
 * 三演示角色（负责人可改 / 成员只读 / 访问者只读）切换，
 * 展示权限对可见性与可编辑性的影响（§9）；五类目卡。
 */
const { t } = useI18n();

const demo = useSettingsDemo();

const icons = {
  projectGeneral: FolderCog,
  projectMembers: UsersRound,
  repositories: GitBranch,
  development: Terminal,
  projectAi: Sparkles
};

const personaOptions = [
  { value: "project-lead", labelKey: "settings.demo.persona.projectLead" },
  { value: "project-member", labelKey: "settings.demo.persona.projectMember" },
  { value: "project-viewer", labelKey: "settings.demo.persona.projectViewer" }
] as const;

const categories = [
  {
    id: "projectGeneral",
    descKey: "settings.project.landing.categoryGeneral",
    to: DESIGN_PATHS.settings.projectGeneral
  },
  {
    id: "projectMembers",
    descKey: "settings.project.landing.categoryMembers",
    to: DESIGN_PATHS.settings.projectMembers
  },
  {
    id: "repositories",
    descKey: "settings.project.landing.categoryRepositories",
    to: DESIGN_PATHS.settings.projectRepositories
  },
  {
    id: "development",
    descKey: "settings.project.landing.categoryDevelopment",
    to: DESIGN_PATHS.settings.projectDevelopment
  },
  {
    id: "projectAi",
    descKey: "settings.project.landing.categoryAi",
    to: DESIGN_PATHS.settings.projectAi
  }
] as const;

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.architecture.title"),
    to: DESIGN_PATHS.settings.architecture
  },
  { label: t("settings.project.landing.title") }
];
</script>

<template>
  <SettingsPageLayout
    :breadcrumb="breadcrumb"
    :title="t('settings.project.landing.title')"
    :description="t('settings.project.landing.desc')"
    :icons="icons"
  >
    <template #actions>
      <div class="project-landing__persona">
        <span class="project-landing__persona-label">
          {{ t("settings.demo.personaLabel") }}
        </span>
        <Radio
          v-for="option in personaOptions"
          :key="option.value"
          :model-value="demo.state.projectPersona"
          :value="option.value"
          :label="t(option.labelKey)"
          name="project-persona"
          @update:model-value="
            demo.setProjectPersona(
              $event as 'project-lead' | 'project-member' | 'project-viewer'
            )
          "
        />
      </div>
    </template>

    <template #sidebar>
      <SettingsSidebar
        :scope-label="t('settings.project.landing.title')"
        :entries="PROJECT_NAV"
        :icons="icons"
      />
    </template>

    <p class="project-landing__entry">
      {{ t("settings.architecture.scope.project.entry") }}
      · {{ t("settings.project.landing.personaHint") }}
    </p>
    <section class="project-landing__grid">
      <RouterLink
        v-for="category in categories"
        :key="category.id"
        class="project-landing__card"
        :to="category.to"
      >
        <component
          :is="icons[category.id]"
          class="project-landing__icon"
          aria-hidden="true"
        />
        <h2 class="project-landing__card-title">
          {{ t(`settings.nav.${category.id}`) }}
        </h2>
        <p class="project-landing__card-desc">
          {{ t(category.descKey) }}
        </p>
      </RouterLink>
    </section>
  </SettingsPageLayout>
</template>

<style scoped>
.project-landing__persona {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

.project-landing__persona-label {
  font-size: var(--ws-font-size-micro);
  color: var(--ws-text-secondary);
}

.project-landing__entry {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-accent);
}

.project-landing__grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

.project-landing__card {
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

.project-landing__card:hover {
  border-color: var(--ws-accent);
}

.project-landing__icon {
  width: var(--size-icon-md);
  height: var(--size-icon-md);
  color: var(--ws-accent);
}

.project-landing__card-title {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.project-landing__card-desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.project-landing__card:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

@media (prefers-reduced-motion: reduce) {
  .project-landing__card {
    transition: none;
  }
}
</style>
