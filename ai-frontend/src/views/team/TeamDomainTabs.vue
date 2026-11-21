<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";

/**
 * Team 域页内导航（P4-ORG UX 修订 V1.0 / Figma T 系列「Page navigation」）。
 *
 * Overview | Members 两个页签横贯团队域页面（概览 / 成员 / Owner 继任 /
 * 受限态）：Overview = /dashboard，Members = /settings/team/members；
 * 成员子页（Owner 继任）随 Members 激活。激活态 = 下划线，不做禁用。
 */
const { t } = useI18n();
const route = useRoute();

const tabs = computed(() => [
  { key: "overview", to: "/dashboard", label: t("team.tabs.overview") },
  {
    key: "members",
    to: "/settings/team/members",
    label: t("team.tabs.members")
  }
]);

function isActive(tab: { key: string; to: string }): boolean {
  if (tab.key === "members") {
    return route.path === tab.to || route.path.startsWith(`${tab.to}/`);
  }
  return route.path === tab.to;
}
</script>

<template>
  <nav class="team-tabs" :aria-label="t('team.tabs.label')">
    <RouterLink
      v-for="tab in tabs"
      :key="tab.key"
      :to="tab.to"
      class="team-tabs__item"
      :class="{ 'is-active': isActive(tab) }"
      :aria-current="isActive(tab) ? 'page' : undefined"
    >
      {{ tab.label }}
    </RouterLink>
  </nav>
</template>

<style scoped>
.team-tabs {
  display: flex;
  align-items: stretch;
  border-bottom: 1px solid var(--ws-border);
}

.team-tabs__item {
  display: flex;
  align-items: center;
  padding: 0 var(--space-3);
  height: 2.25rem;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
  text-decoration: none;
}

.team-tabs__item:hover {
  color: var(--text-primary);
}

.team-tabs__item.is-active {
  border-bottom-color: var(--ws-accent);
  color: var(--text-primary);
}

.team-tabs__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}
</style>
