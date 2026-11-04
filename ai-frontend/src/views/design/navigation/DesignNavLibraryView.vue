<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Folders } from "lucide-vue-next";

import { SidebarItem } from "@/components/ui/sidebar-item";
import { NavigationGroup } from "@/components/ui/navigation-group";
import { Breadcrumb } from "@/components/ui/breadcrumb";
import { Tooltip } from "@/components/ui/tooltip";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/**
 * /design/navigation/library —— Figma 板 08（导航组件库，9 族 24 specimen）。
 * Navigation Item 六态 / Badge 两类 / Navigation Group 两态 / Divider /
 * Tooltip / Workspace·Project Context Header / Sidebar Collapse / Breadcrumb。
 */
const { t } = useI18n();

const crumbs = [
  { label: "Projects" },
  { label: "Atlas" },
  { label: "Repositories" }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.navigation.library.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.navigation.library.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>NAVIGATION ITEM / 36px · 16px icon · 8px radius</h2>
      <div class="design-page__row">
        <Specimen label="State=Default">
          <SidebarItem label="Projects" :icon="Folders" style="width: 280px" />
        </Specimen>
        <Specimen label="State=Active">
          <SidebarItem
            label="Projects"
            :icon="Folders"
            state="active"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="State=Restricted">
          <SidebarItem
            label="Projects"
            :icon="Folders"
            state="restricted"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="State=Loading">
          <SidebarItem label="Projects" state="loading" style="width: 280px" />
        </Specimen>
        <Specimen label="Badge=Count">
          <SidebarItem
            label="Projects"
            :icon="Folders"
            badge="8"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="Badge=Status">
          <SidebarItem
            label="Projects"
            :icon="Folders"
            badge="Ready"
            badge-type="status"
            style="width: 280px"
          />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>NAVIGATION GROUP / EXPANDED · COLLAPSED</h2>
      <div class="design-page__row">
        <Specimen label="State=Expanded" width="280px">
          <div class="library__navcol">
            <NavigationGroup title="PROJECT">
              <SidebarItem label="Overview" state="active" />
              <SidebarItem label="Repositories" />
            </NavigationGroup>
          </div>
        </Specimen>
        <Specimen label="State=Collapsed" width="280px">
          <div class="library__navcol">
            <NavigationGroup title="PROJECT" default-collapsed />
          </div>
        </Specimen>
        <Specimen label="NAVIGATION DIVIDER / Default" width="280px">
          <hr class="library__divider" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>TOOLTIP / WORKSPACE·PROJECT CONTEXT HEADER / COLLAPSE BUTTON</h2>
      <div class="design-page__row">
        <Specimen label="Tooltip / Default">
          <Tooltip content="Atlas Platform · Development">
            <button type="button" class="library__tipbtn">Hover me</button>
          </Tooltip>
        </Specimen>
        <Specimen label="Workspace Context Header / Selected" width="280px">
          <div class="library__header">
            <span class="library__header-title">Atlas Platform</span>
            <span class="library__header-sub">Workspace</span>
          </div>
        </Specimen>
        <Specimen label="Project Context Header / Selected" width="280px">
          <div class="library__header">
            <span class="library__header-title">Research Sandbox</span>
            <span class="library__header-sub">Project · 5 repositories</span>
          </div>
        </Specimen>
        <Specimen label="Sidebar Collapse Button / Expanded · Collapsed">
          <div class="library__collapse-row">
            <button
              type="button"
              class="library__collapse"
              aria-label="Collapse sidebar"
            >
              «
            </button>
            <button
              type="button"
              class="library__collapse"
              aria-label="Expand sidebar"
            >
              »
            </button>
          </div>
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>BREADCRUMB / Default</h2>
      <div class="design-page__row">
        <Specimen label="Breadcrumb / Default" width="360px">
          <Breadcrumb :items="crumbs" />
        </Specimen>
      </div>
    </section>
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

.design-page__group {
  margin: 0 0 var(--space-8);
}

.design-page__group h2 {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--space-4);
}

.library__navcol {
  padding: var(--space-2);
  background-color: var(--ws-sidebar);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.library__divider {
  margin: 0;
  border: 0;
  border-top: var(--hairline) solid var(--ws-border);
}

.library__tipbtn {
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  background-color: var(--ws-surface);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-body);
  cursor: pointer;
}

.library__tipbtn:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.library__header {
  display: flex;
  flex-direction: column;
  gap: 2px;
  height: var(--size-context-header);
  justify-content: center;
  padding: 0 var(--space-3);
  background-color: var(--accent-surface);
  border-radius: var(--radius-sm);
}

.library__header-title {
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-medium);
  color: var(--ws-text-primary);
}

.library__header-sub {
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.library__collapse-row {
  display: flex;
  gap: var(--space-2);
}

.library__collapse {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-compact);
  height: var(--size-control-compact);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  color: var(--ws-text-secondary);
  cursor: pointer;
}

.library__collapse:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}
</style>
