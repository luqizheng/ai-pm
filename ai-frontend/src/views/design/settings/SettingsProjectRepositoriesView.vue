<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowUpRight, GitBranch } from "lucide-vue-next";

import {
  PROJECT_NAV,
  PERSONA_LABEL_KEYS,
  SettingsConfirmationDialog,
  SettingsFormSection,
  SettingsPermissionNotice,
  SettingsReadOnlyField,
  PROJECT_REPOSITORIES_FIXTURE,
  useSettingsDemo,
  type DemoRepository
} from "@/components/settings";
import SettingsFrame from "./SettingsFrame.vue";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 15 — 项目仓库关联（§4C Repositories）。
 * 关联仓库列表 + 主仓库标记 + 设主仓库（确认）；
 * 仓库管理入口卡：链接跳转不复刻管理界面（§4C/§10 约束）。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const editable = computed(
  () => demo.projectAccess.value.repositories === "editable"
);
const personaLabel = computed(() =>
  t(PERSONA_LABEL_KEYS[demo.state.projectPersona])
);

const repositories = ref<DemoRepository[]>(PROJECT_REPOSITORIES_FIXTURE);
const primaryTarget = ref<DemoRepository | null>(null);
const primaryOpen = computed(() => primaryTarget.value !== null);

function confirmSetPrimary() {
  if (primaryTarget.value) {
    for (const repo of repositories.value) {
      repo.primary = repo.id === primaryTarget.value.id;
    }
  }
  primaryTarget.value = null;
}

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.project.landing.title"),
    to: DESIGN_PATHS.settings.project
  },
  { label: t("settings.nav.repositories") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.repositories')"
    :scope-label="t('settings.project.landing.title')"
    :entries="PROJECT_NAV"
  >
    <SettingsPermissionNotice
      v-if="!editable"
      access="readonly"
      :role="personaLabel"
    />

    <SettingsFormSection
      :title="t('settings.project.repositories.title')"
      :description="t('settings.project.repositories.desc')"
    >
      <div class="project-repos__list">
        <Card v-for="repo in repositories" :key="repo.id">
          <div class="project-repos__row">
            <div class="project-repos__info">
              <div class="project-repos__name">
                <GitBranch class="project-repos__icon" aria-hidden="true" />
                <span>{{ repo.name }}</span>
                <Badge v-if="repo.primary" variant="accent" size="sm">
                  {{ t("settings.project.repositories.primaryBadge") }}
                </Badge>
              </div>
              <SettingsReadOnlyField
                :value="repo.path"
                :label="t('settings.project.repositories.path')"
              />
            </div>
            <div
              v-if="editable && !repo.primary"
              class="project-repos__actions"
            >
              <Button
                variant="secondary"
                size="sm"
                @click="primaryTarget = repo"
              >
                {{ t("settings.project.repositories.setPrimary") }}
              </Button>
            </div>
          </div>
        </Card>
      </div>
    </SettingsFormSection>

    <Card>
      <div class="project-repos__manage">
        <div class="project-repos__manage-text">
          <p class="project-repos__manage-title">
            {{ t("settings.project.repositories.manageTitle") }}
          </p>
          <p class="project-repos__manage-desc">
            {{ t("settings.project.repositories.manageDesc") }}
          </p>
        </div>
        <Button variant="secondary" size="sm">
          <span class="project-repos__manage-link">
            {{ t("settings.project.repositories.manageLink") }}
            <ArrowUpRight class="project-repos__icon" aria-hidden="true" />
          </span>
        </Button>
      </div>
    </Card>

    <SettingsConfirmationDialog
      :open="primaryOpen"
      :title="t('settings.project.repositories.setPrimaryTitle')"
      :description="t('settings.project.repositories.setPrimaryDesc')"
      :confirm-label="t('settings.project.repositories.setPrimary')"
      @update:open="
        value => {
          if (!value) primaryTarget = null;
        }
      "
      @confirm="confirmSetPrimary"
    />
  </SettingsFrame>
</template>

<style scoped>
.project-repos__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.project-repos__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.project-repos__info {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  flex: 1;
}

.project-repos__name {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.project-repos__icon {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  color: var(--ws-text-secondary);
}

.project-repos__actions {
  display: flex;
  flex: none;
}

.project-repos__manage {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.project-repos__manage-text {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.project-repos__manage-title {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.project-repos__manage-desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.project-repos__manage-link {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}
</style>
