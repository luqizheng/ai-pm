<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  BookOpen,
  Folders,
  GitBranch,
  Inbox,
  LayoutDashboard,
  ListChecks,
  ScanSearch,
  Settings2,
  Sparkles,
  Terminal,
  UserRound,
  UsersRound
} from "lucide-vue-next";

import { SidebarItem } from "@/components/ui/sidebar-item";
import { NavigationGroup } from "@/components/ui/navigation-group";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/**
 * /design/navigation/contexts —— Figma 板 03（Personal）/ 04（Project）/ 05（Administration）。
 * 三种导航上下文**不混成一张扁平菜单**（Shell V1.1 §2B）：
 * 用 DS 组件（SidebarItem + NavigationGroup）分别组合三列，呈现各上下文的分组差异。
 */
const { t } = useI18n();
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.navigation.contexts.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.navigation.contexts.subtitle") }}
      </p>
    </template>

    <div class="design-page__row">
      <Specimen
        label="Board 03 — Personal Workspace Navigation (7:15225)"
        width="320px"
      >
        <div class="contexts__col">
          <NavigationGroup title="PERSONAL" default-collapsed>
            <SidebarItem
              label="Personal workspace"
              :icon="UserRound"
              state="active"
            />
            <SidebarItem label="Inbox" badge="3" />
          </NavigationGroup>
          <NavigationGroup title="GLOBAL" default-collapsed>
            <SidebarItem label="Dashboard" :icon="LayoutDashboard" />
            <SidebarItem label="Projects" :icon="Folders" />
          </NavigationGroup>
        </div>
      </Specimen>

      <Specimen label="Board 04 — Project Navigation (7:15465)" width="320px">
        <div class="contexts__col">
          <NavigationGroup title="PROJECT OVERVIEW">
            <SidebarItem label="Overview" :icon="LayoutDashboard" />
          </NavigationGroup>
          <NavigationGroup title="DEVELOPMENT">
            <SidebarItem
              label="Repositories"
              :icon="GitBranch"
              state="active"
              badge="12"
            />
            <SidebarItem label="Code discovery" :icon="ScanSearch" />
          </NavigationGroup>
          <NavigationGroup title="WORK ITEMS">
            <SidebarItem label="Requirements" :icon="ListChecks" />
            <SidebarItem label="AI execution" :icon="Sparkles" badge="2" />
            <SidebarItem label="Knowledge" :icon="BookOpen" />
            <SidebarItem
              label="Dev environment"
              :icon="Terminal"
              badge-type="status"
              badge="Ready"
            />
          </NavigationGroup>
        </div>
      </Specimen>

      <Specimen
        label="Board 05 — Workspace Administration (7:15731)"
        width="320px"
      >
        <div class="contexts__col">
          <NavigationGroup title="ADMINISTRATION">
            <SidebarItem label="General" :icon="Settings2" state="active" />
            <SidebarItem label="Members" :icon="UsersRound" />
            <SidebarItem label="Security" />
            <SidebarItem label="Integrations" />
          </NavigationGroup>
          <NavigationGroup title="GLOBAL" default-collapsed>
            <SidebarItem label="Inbox" :icon="Inbox" />
          </NavigationGroup>
        </div>
      </Specimen>
    </div>

    <p class="design-page__hint">
      {{ t("design.notes.contexts") }}
    </p>
  </DesignLayout>
</template>

<style scoped>
.design-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--space-4);
}

.design-page__hint {
  margin-top: var(--space-4);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.contexts__col {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-2);
  background-color: var(--ws-sidebar);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}
</style>
