<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

/**
 * 产品级页内导航（P4-ORG UX 修订 / Figma O1「Page navigation」）。
 *
 * Overview | Members | Projects 三页签可跳转（路由随 productId）；设计稿的
 * Requirements / Planning、Knowledge、Activity 为后续域入口，按壳约定渲染
 * **planned 禁用页签**（可见不可点，不伪装已上线）。Lifecycle 不是页签——
 * 经概览页「View lifecycle」按钮进入。
 */
const props = defineProps<{
  productId: string;
}>();

const { t } = useI18n();
const route = useRoute();

const tabs = computed(() => [
  {
    key: "overview",
    to: `/settings/team/products/${props.productId}`,
    label: t("team.products.tabOverview")
  },
  {
    key: "members",
    to: `/settings/team/products/${props.productId}/members`,
    label: t("team.products.tabMembers")
  },
  {
    key: "projects",
    to: `/settings/team/products/${props.productId}/projects`,
    label: t("team.products.tabProjects")
  }
]);

const plannedTabs = computed(() => [
  { key: "requirements", label: t("team.products.tabRequirements") },
  { key: "knowledge", label: t("team.products.tabKnowledge") },
  { key: "activity", label: t("team.products.tabActivity") }
]);

function isActive(tab: { key: string; to: string }): boolean {
  if (tab.key === "overview") {
    return route.path === tab.to;
  }
  return route.path === tab.to || route.path.startsWith(`${tab.to}/`);
}
</script>

<template>
  <nav class="product-tabs" :aria-label="t('team.products.tabsLabel')">
    <RouterLink
      v-for="tab in tabs"
      :key="tab.key"
      :to="tab.to"
      class="product-tabs__item"
      :class="{ 'is-active': isActive(tab) }"
      :aria-current="isActive(tab) ? 'page' : undefined"
    >
      {{ tab.label }}
    </RouterLink>
    <span
      v-for="tab in plannedTabs"
      :key="tab.key"
      class="product-tabs__item product-tabs__item--planned"
    >
      {{ tab.label }}
      <span class="product-tabs__planned-badge">{{
        t("shell.app.side.plannedBadge")
      }}</span>
    </span>
  </nav>
</template>

<style scoped>
.product-tabs {
  display: flex;
  align-items: stretch;
  border-bottom: 1px solid var(--ws-border);
}

.product-tabs__item {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0 var(--space-3);
  height: 2.25rem;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
  text-decoration: none;
  cursor: pointer;
}

.product-tabs__item:not(.product-tabs__item--planned):hover {
  color: var(--text-primary);
}

.product-tabs__item.is-active {
  border-bottom-color: var(--ws-accent);
  color: var(--text-primary);
}

.product-tabs__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.product-tabs__item--planned {
  color: var(--text-tertiary);
  cursor: not-allowed;
  opacity: 0.7;
}

.product-tabs__planned-badge {
  padding: 0 var(--space-1);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
