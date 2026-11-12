<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

/**
 * 执行规划的二级视图切换条（依赖图 / 时间线）。
 *
 * 并行组不单设路由：它是依赖图页的第二视图（S04-UI-05 §7），
 * 由图页内部的分段控件切换。条形样式沿用模块页签条口径。
 */
const { t } = useI18n();
const route = useRoute();

const VIEW_TABS = [
  {
    key: "graph",
    path: "/project/tasks/planning/graph",
    labelKey: "s04.tasks.planning.viewGraph"
  },
  {
    key: "timeline",
    path: "/project/tasks/planning/timeline",
    labelKey: "s04.tasks.planning.viewTimeline"
  }
] as const;

const activeKey = computed(
  () => VIEW_TABS.find(tab => tab.path === route.path)?.key ?? ""
);
</script>

<template>
  <nav class="pl-views" :aria-label="t('s04.tasks.page.planningTitle')">
    <RouterLink
      v-for="tab in VIEW_TABS"
      :key="tab.key"
      :to="tab.path"
      class="pl-views__tab"
      :class="{ 'is-active': activeKey === tab.key }"
      :aria-current="activeKey === tab.key ? 'page' : undefined"
      :data-testid="`pl-view-${tab.key}`"
    >
      {{ t(tab.labelKey) }}
    </RouterLink>
  </nav>
</template>

<style scoped>
.pl-views {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.pl-views__tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--ws-text-secondary);
  text-decoration: none;
  white-space: nowrap;
  transition:
    color var(--duration-fast) var(--ease-decelerate),
    background-color var(--duration-fast) var(--ease-decelerate);
}

.pl-views__tab:hover {
  background-color: var(--surface-base);
}

.pl-views__tab:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
  border-radius: var(--radius-xs);
}

.pl-views__tab.is-active {
  color: var(--ws-text-primary);
}

.pl-views__tab.is-active::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--ws-accent);
  content: "";
}
</style>
