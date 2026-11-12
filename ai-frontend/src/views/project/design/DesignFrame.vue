<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { getDesignWorkspace, type DesignWorkspace } from "@/api/designs";
import { useDesignProject } from "./use-design-project";

/**
 * S03 设计与技术决策工作空间壳（四页共用）。
 *
 * 组织方式同 S04 任务管理（TasksFrame）与 Project Setting · AI Team
 * （AiTeamFrame）：四个子页是**独立路由**而非同页切换，页签条是 RouterLink
 * ——保留浏览器前进/后退与深链语义。
 *
 * 壳层同时承载设计稿 §8 的「统一上下文区域」：项目内设计/ADR/基线计数、
 * 待办审批数、未闭评审问题数、需求覆盖与过期绑定数。四页的页头与空态判据
 * 都读这一份聚合，避免每页各打一组计数请求。
 */
const { t } = useI18n();
const route = useRoute();
const { projectId, hasProject } = useDesignProject();

const DESIGN_TABS = [
  {
    key: "analysis",
    path: "/project/design",
    labelKey: "s03.design.tabs.analysis"
  },
  {
    key: "designs",
    path: "/project/design/technical-designs",
    labelKey: "s03.design.tabs.designs"
  },
  {
    key: "adrs",
    path: "/project/design/adrs",
    labelKey: "s03.design.tabs.adrs"
  },
  {
    key: "readiness",
    path: "/project/design/readiness",
    labelKey: "s03.design.tabs.readiness"
  }
] as const;

const activeKey = computed(
  () => DESIGN_TABS.find(tab => tab.path === route.path)?.key ?? ""
);

const workspace = ref<DesignWorkspace | null>(null);

async function loadWorkspace() {
  if (!hasProject.value) {
    workspace.value = null;
    return;
  }
  try {
    workspace.value = await getDesignWorkspace(projectId.value);
  } catch {
    workspace.value = null;
  }
}

onMounted(loadWorkspace);
watch(projectId, loadWorkspace);

const contextItems = computed(() => {
  const data = workspace.value;
  if (!data) return [];
  return [
    {
      key: "designs",
      label: t("s03.design.context.designs"),
      value: String(data.design_count)
    },
    {
      key: "adrs",
      label: t("s03.design.context.adrs"),
      value: String(data.adr_count)
    },
    {
      key: "baselines",
      label: t("s03.design.context.baselines"),
      value: String(data.baseline_count)
    },
    {
      key: "pendingApprovals",
      label: t("s03.design.context.pendingApprovals"),
      value: String(data.pending_approval_count),
      tone: data.pending_approval_count > 0 ? "warning" : "neutral"
    },
    {
      key: "openFindings",
      label: t("s03.design.context.openFindings"),
      value: String(data.open_finding_count),
      tone: data.open_finding_count > 0 ? "warning" : "neutral"
    },
    {
      key: "linkedRequirements",
      label: t("s03.design.context.linkedRequirements"),
      value: String(data.linked_requirement_count),
      tone: "neutral"
    },
    {
      key: "staleRequirements",
      label: t("s03.design.context.staleRequirements"),
      value: String(data.stale_requirement_count),
      tone: data.stale_requirement_count > 0 ? "danger" : "neutral"
    }
  ];
});
</script>

<template>
  <div class="design-workspace">
    <nav class="design-tabs" :aria-label="t('s03.design.module')">
      <RouterLink
        v-for="tab in DESIGN_TABS"
        :key="tab.key"
        :to="tab.path"
        class="design-tabs__tab"
        :class="{ 'is-active': activeKey === tab.key }"
        :aria-current="activeKey === tab.key ? 'page' : undefined"
        :data-testid="`design-tab-${tab.key}`"
      >
        {{ t(tab.labelKey) }}
      </RouterLink>
    </nav>

    <section
      v-if="contextItems.length > 0"
      class="design-context"
      :aria-label="t('s03.design.context.title')"
      data-testid="design-context"
    >
      <span class="design-context__title">{{
        t("s03.design.context.title")
      }}</span>
      <span
        v-for="item in contextItems"
        :key="item.key"
        class="design-context__item"
        :class="`is-${item.tone}`"
        :data-testid="`design-context-${item.key}`"
      >
        <span class="design-context__label">{{ item.label }}</span>
        <span class="design-context__value">{{ item.value }}</span>
      </span>
    </section>

    <slot />
  </div>
</template>

<style scoped>
.design-workspace {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

/* 页签条口径同 DS/Navigation/Tab：透明底行 + active 底缘 2px accent 指示条 */
.design-tabs {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.design-tabs__tab {
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

.design-tabs__tab:hover {
  background-color: var(--surface-base);
  color: var(--ws-text-secondary);
}

.design-tabs__tab:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
  border-radius: var(--radius-xs);
}

.design-tabs__tab.is-active {
  color: var(--ws-text-primary);
}

.design-tabs__tab.is-active::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--ws-accent);
  content: "";
}

/* 统一上下文区域：紧凑单行指标条 */
.design-context {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-lg);
}

.design-context__title {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-secondary);
}

.design-context__item {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.design-context__label {
  color: var(--ws-text-secondary);
}

.design-context__value {
  font-variant-numeric: tabular-nums;
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}
</style>
