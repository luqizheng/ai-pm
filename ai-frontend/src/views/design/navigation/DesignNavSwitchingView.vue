<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  BookOpen,
  GitBranch,
  ScanSearch,
  Sparkles,
  Terminal
} from "lucide-vue-next";

import { SidebarItem } from "@/components/ui/sidebar-item";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/**
 * /design/navigation/switching —— Figma 板 07（项目切换序列，静态差异对照）。
 * Before（Atlas）/ Switching（加载行）/ After（Research Sandbox）三列，
 * 只渲染序列中**变化**的行（板 07 规格：角标 2→1、状态点消失、Loading 行、
 * 上下文文案随项目改写）。
 */
const { t } = useI18n();

const beforeItems = [
  {
    label: "Repositories",
    icon: GitBranch,
    badge: "12" as const,
    active: true
  },
  { label: "Code discovery", icon: ScanSearch },
  { label: "AI execution", icon: Sparkles, badge: 2 as number },
  { label: "Knowledge", icon: BookOpen },
  { label: "Dev environment", icon: Terminal, status: "Ready" }
];

const switchingItems = [
  {
    label: "Repositories",
    icon: GitBranch,
    badge: "12" as const,
    active: true
  },
  { label: "Code discovery", icon: ScanSearch },
  { label: "AI execution", icon: Sparkles, badge: 1 as number },
  { label: "Knowledge", icon: BookOpen },
  {
    label: "Dev environment",
    icon: Terminal,
    status: undefined as string | undefined
  },
  { label: "Switching project…", loading: true }
];

const afterItems = [
  { label: "Repositories", icon: GitBranch, badge: "5" as const, active: true },
  { label: "Code discovery", icon: ScanSearch },
  { label: "AI execution", icon: Sparkles, badge: 1 as number },
  { label: "Knowledge", icon: BookOpen },
  {
    label: "Dev environment",
    icon: Terminal,
    status: undefined as string | undefined
  }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.navigation.switching.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.navigation.switching.subtitle") }}
      </p>
    </template>

    <div class="design-page__row">
      <Specimen label="Before — Atlas Platform" width="320px">
        <div class="switching__col">
          <SidebarItem
            v-for="item in beforeItems"
            :key="item.label"
            :label="item.label"
            :icon="item.icon"
            :state="item.active ? 'active' : 'default'"
            :badge="item.badge ?? item.status"
            :badge-type="item.status ? 'status' : 'count'"
            style="background: transparent"
          />
        </div>
      </Specimen>

      <Specimen label="Switching — Loading" width="320px">
        <div class="switching__col">
          <template v-for="item in switchingItems" :key="item.label">
            <SidebarItem
              v-if="!item.loading"
              :label="item.label"
              :icon="item.icon"
              :state="item.active ? 'active' : 'default'"
              :badge="item.badge ?? item.status"
              :badge-type="item.status ? 'status' : 'count'"
            />
            <SidebarItem v-else label="Switching project…" state="loading" />
          </template>
        </div>
      </Specimen>

      <Specimen label="After — Research Sandbox" width="320px">
        <div class="switching__col">
          <SidebarItem
            v-for="item in afterItems"
            :key="item.label"
            :label="item.label"
            :icon="item.icon"
            :state="item.active ? 'active' : 'default'"
            :badge="item.badge ?? item.status"
            :badge-type="item.status ? 'status' : 'count'"
          />
        </div>
      </Specimen>
    </div>

    <p class="design-page__hint">
      {{ t("design.notes.switching") }}
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

.switching__col {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2);
  background-color: var(--ws-sidebar);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}
</style>
