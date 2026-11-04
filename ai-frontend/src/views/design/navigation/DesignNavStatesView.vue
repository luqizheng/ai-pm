<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Folder, Inbox } from "lucide-vue-next";

import { SidebarItem } from "@/components/ui/sidebar-item";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/**
 * /design/navigation/states —— Figma 板 06（交互状态全集）：
 * Navigation Item 六态 / 徽标两类 / Rail 折叠态。
 */
const { t } = useI18n();
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.navigation.states.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.navigation.states.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>NAVIGATION ITEM / 36px · 16px icon · 8px radius (board 08)</h2>
      <div class="design-page__row">
        <Specimen label="State=Default">
          <SidebarItem label="Projects" :icon="Folder" style="width: 280px" />
        </Specimen>
        <Specimen label="State=Hover (live)">
          <SidebarItem label="Projects" :icon="Folder" style="width: 280px" />
        </Specimen>
        <Specimen label="State=Active">
          <SidebarItem
            label="Projects"
            :icon="Folder"
            state="active"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="State=Focus (via Tab)">
          <SidebarItem label="Projects" :icon="Folder" style="width: 280px" />
        </Specimen>
        <Specimen label="State=Restricted">
          <SidebarItem
            label="Projects"
            :icon="Folder"
            state="restricted"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="State=Loading">
          <SidebarItem label="Projects" state="loading" style="width: 280px" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>NAVIGATION BADGE (board 08: Type=Count / Type=Status)</h2>
      <div class="design-page__row">
        <Specimen label="Count / unread">
          <SidebarItem
            label="Inbox"
            :icon="Inbox"
            badge="3"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="Status / environment ready">
          <SidebarItem
            label="Dev environment"
            badge="Ready"
            badge-type="status"
            style="width: 280px"
          />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>RAIL (board 08: Rail=Collapsed · 60px)</h2>
      <div class="design-page__row">
        <Specimen label="State=Active, Rail=Collapsed">
          <div
            style="
              width: 60px;
              background: var(--ws-sidebar);
              border-radius: var(--radius-sm);
              padding: var(--space-1);
            "
          >
            <SidebarItem
              label="Projects"
              :icon="Folder"
              state="active"
              collapsed
            />
          </div>
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
