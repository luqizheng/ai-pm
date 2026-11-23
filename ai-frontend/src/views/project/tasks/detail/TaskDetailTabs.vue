<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

/**
 * 任务详情四页签条（PA-07 179:3603 B 页 action 条口径）：
 * Overview / Dependencies / Revision history / Runs——独立路由 RouterLink
 * （TasksFrame 同组织），任务级深链保留浏览器语义。
 */
const props = defineProps<{ taskId: string }>();
const { t } = useI18n();
const route = useRoute();

const DETAIL_TABS = computed(() => [
  {
    key: "overview",
    path: `/project/tasks/${props.taskId}`,
    labelKey: "s04.tasks.page.detailTitle"
  },
  {
    key: "dependencies",
    path: `/project/tasks/${props.taskId}/dependencies`,
    labelKey: "pa07.dependencies.title"
  },
  {
    key: "history",
    path: `/project/tasks/${props.taskId}/history`,
    labelKey: "pa07.history.title"
  },
  {
    key: "runs",
    path: `/project/tasks/${props.taskId}/runs`,
    labelKey: "pa07.runs.title"
  }
]);

const activeKey = computed(
  () =>
    DETAIL_TABS.value.find(tab => tab.path === route.path)?.key ?? "overview"
);
</script>

<template>
  <nav
    class="task-detail-tabs"
    :data-testid="`task-detail-tabs-${props.taskId}`"
  >
    <RouterLink
      v-for="tab in DETAIL_TABS"
      :key="tab.key"
      :to="tab.path"
      class="task-detail-tabs__tab"
      :class="{ 'is-active': activeKey === tab.key }"
      :aria-current="activeKey === tab.key ? 'page' : undefined"
      :data-testid="`task-detail-tab-${tab.key}`"
    >
      {{ t(tab.labelKey) }}
    </RouterLink>
  </nav>
</template>

<style scoped>
/* 口径同 TasksFrame 页签条：透明底行 + active 底缘指示条 */
.task-detail-tabs {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.task-detail-tabs__tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-text-secondary);
  text-decoration: none;
  white-space: nowrap;
}

.task-detail-tabs__tab:hover {
  background-color: var(--surface-base);
}

.task-detail-tabs__tab.is-active {
  color: var(--ws-text-primary);
}

.task-detail-tabs__tab.is-active::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--ws-accent);
  content: "";
}
</style>
