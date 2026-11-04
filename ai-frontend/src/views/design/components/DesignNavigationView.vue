<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Folder,
  GitBranch,
  LayoutDashboard,
  Settings2,
  UsersRound
} from "lucide-vue-next";

import { SidebarItem } from "@/components/ui/sidebar-item";
import { NavigationGroup } from "@/components/ui/navigation-group";
import { TopBarItem } from "@/components/ui/top-bar-item";
import { Breadcrumb } from "@/components/ui/breadcrumb";
import { Tabs } from "@/components/ui/tabs";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/** /design/components/navigation —— DS/Navigation 族样板 */
const { t } = useI18n();

const groupOpen = ref(true);
const groupClosed = ref(false);
const tab = ref("overview");

const crumbs = [
  { label: "Projects", to: undefined as string | undefined },
  { label: "Atlas", to: undefined },
  { label: "Repositories" }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.components.navigation.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.components.navigation.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>SidebarItem (13:33329)</h2>
      <div class="design-page__row">
        <Specimen label="State=Default">
          <SidebarItem label="Projects" :icon="Folder" style="width: 216px" />
        </Specimen>
        <Specimen label="State=Active">
          <SidebarItem
            label="Dashboard"
            :icon="LayoutDashboard"
            state="active"
            style="width: 216px"
          />
        </Specimen>
        <Specimen label="State=Restricted">
          <SidebarItem
            label="Admin console"
            :icon="Settings2"
            state="restricted"
            style="width: 216px"
          />
        </Specimen>
        <Specimen label="State=Loading">
          <SidebarItem label="Syncing" state="loading" style="width: 216px" />
        </Specimen>
        <Specimen label="Badge=Count">
          <SidebarItem label="Inbox" badge="3" style="width: 216px" />
        </Specimen>
        <Specimen label="Badge=Status">
          <SidebarItem
            label="Environment"
            badge="Ready"
            badge-type="status"
            style="width: 216px"
          />
        </Specimen>
        <Specimen label="Rail=Collapsed">
          <SidebarItem
            label="Projects"
            :icon="Folder"
            collapsed
            style="width: 60px"
          />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>NavigationGroup (13:33391)</h2>
      <div class="design-page__row">
        <Specimen label="State=Expanded" width="240px">
          <NavigationGroup
            v-model:collapsed="groupOpen"
            title="PROJECT"
            style="
              background: var(--surface-sunken);
              border-radius: var(--radius-sm);
              padding: var(--space-1);
            "
          >
            <SidebarItem
              label="Overview"
              :icon="LayoutDashboard"
              state="active"
            />
            <SidebarItem label="Repositories" :icon="GitBranch" />
            <SidebarItem label="Members" :icon="UsersRound" />
          </NavigationGroup>
        </Specimen>
        <Specimen label="State=Collapsed" width="240px">
          <NavigationGroup
            v-model:collapsed="groupClosed"
            title="ADMINISTRATION"
            :badge="4"
            style="
              background: var(--surface-sunken);
              border-radius: var(--radius-sm);
              padding: var(--space-1);
            "
          />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>TopBarItem (13:33520)</h2>
      <div class="design-page__row">
        <Specimen label="Default">
          <TopBarItem label="Notifications" :icon="Folder" shortcut="⌘ N" />
        </Specimen>
        <Specimen label="Active">
          <TopBarItem label="AI" active />
        </Specimen>
        <Specimen label="Badge">
          <TopBarItem label="Inbox" badge="3" />
        </Specimen>
        <Specimen
          v-for="s in ['sm', 'md', 'lg'] as const"
          :key="s"
          :label="`Size=${s}`"
        >
          <TopBarItem label="Help" :size="s" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>Breadcrumb (13:33452) / Tab (13:33477)</h2>
      <div class="design-page__row">
        <Specimen label="Breadcrumb / Default" width="280px">
          <Breadcrumb :items="crumbs" />
        </Specimen>
        <Specimen label="Tab / Default + Active" width="280px">
          <Tabs
            v-model="tab"
            :tabs="[
              { value: 'overview', label: 'Overview' },
              { value: 'branches', label: 'Branches' }
            ]"
          />
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
</style>
