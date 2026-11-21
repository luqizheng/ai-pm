<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

/**
 * S04 任务管理模块的页内页签条（docs/scenarios/s04/ui）。
 *
 * 组织方式同 Project Setting · AI Team（AiTeamFrame）：五个子页是**独立
 * 路由**而非同页切换，页签条是 RouterLink——保留浏览器前进/后退与深链
 * 语义；未落地子页路由由 router/index.ts 落到占位页。
 */
const { t } = useI18n();
const route = useRoute();

const TASK_TABS = [
  {
    key: "list",
    path: "/project/tasks/list",
    labelKey: "s04.tasks.tabs.list"
  },
  {
    key: "overview",
    path: "/project/tasks",
    labelKey: "s04.tasks.tabs.overview"
  },
  {
    key: "breakdown",
    path: "/project/tasks/breakdown",
    labelKey: "s04.tasks.tabs.breakdown"
  },
  {
    key: "backlog",
    path: "/project/tasks/backlog",
    labelKey: "s04.tasks.tabs.backlog"
  },
  {
    key: "planning",
    path: "/project/tasks/planning",
    labelKey: "s04.tasks.tabs.planning"
  },
  {
    key: "readiness",
    path: "/project/tasks/readiness",
    labelKey: "s04.tasks.tabs.readiness"
  }
] as const;

// list 页内二级视图（/project/tasks/relationships）也归 list 页签亮起。
const LIST_TAB_PATHS = ["/project/tasks/list", "/project/tasks/relationships"];

const activeKey = computed(() => {
  if (LIST_TAB_PATHS.includes(route.path)) return "list";
  return TASK_TABS.find(tab => tab.path === route.path)?.key ?? "";
});
</script>

<template>
  <nav class="tasks-tabs" :aria-label="t('shell.app.side.item.taskManagement')">
    <RouterLink
      v-for="tab in TASK_TABS"
      :key="tab.key"
      :to="tab.path"
      class="tasks-tabs__tab"
      :class="{ 'is-active': activeKey === tab.key }"
      :aria-current="activeKey === tab.key ? 'page' : undefined"
      :data-testid="`tasks-tab-${tab.key}`"
    >
      {{ t(tab.labelKey) }}
    </RouterLink>
  </nav>
</template>

<style scoped>
/* 页签条口径同 DS/Navigation/Tab：透明底行 + active 底缘 2px accent 指示条 */
.tasks-tabs {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.tasks-tabs__tab {
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

.tasks-tabs__tab:hover {
  background-color: var(--surface-base);
  color: var(--ws-text-secondary);
}

.tasks-tabs__tab:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
  border-radius: var(--radius-xs);
}

.tasks-tabs__tab.is-active {
  color: var(--ws-text-primary);
}

.tasks-tabs__tab.is-active::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--ws-accent);
  content: "";
}
</style>
