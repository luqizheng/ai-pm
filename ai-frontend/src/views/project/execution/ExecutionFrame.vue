<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import {
  getExecutionOverview,
  type ExecutionOverview
} from "@/api/execution-plans";
import { useDesignProject } from "@/views/project/design/use-design-project";
import { rateToPercent, secondsToText } from "./execution-contract";

/**
 * S05 执行与协作工作空间壳（七页共用）。
 *
 * 组织方式同 S04 任务管理 / S03 技术设计：子页是**独立路由**，页签条是
 * RouterLink——保留深链与前进/后退语义。
 *
 * 壳层承载 S05-01 §3 的页头态势区（Execution Overview）：活跃 Run /
 * 待人工处理 / 待评审 / 失败 / 成功率 / 平均时长 / 设备就绪。七页共用
 * 这一份聚合，避免每页各打一组计数请求。
 */
const { t } = useI18n();
const route = useRoute();
const { projectId, hasProject } = useDesignProject();

const EXECUTION_TABS = [
  {
    key: "center",
    path: "/project/execution",
    labelKey: "s05.execution.tabs.center"
  },
  {
    key: "intervention",
    path: "/project/execution/intervention",
    labelKey: "s05.execution.tabs.intervention"
  },
  {
    key: "authorization",
    path: "/project/execution/authorization",
    labelKey: "s05.execution.tabs.authorization"
  },
  {
    key: "review",
    path: "/project/execution/review",
    labelKey: "s05.execution.tabs.review"
  },
  {
    key: "history",
    path: "/project/execution/history",
    labelKey: "s05.execution.tabs.history"
  }
] as const;

const activeKey = computed(() => {
  if (route.path === "/project/execution") return "center";
  const match = EXECUTION_TABS.find(
    tab => tab.path !== "/project/execution" && route.path.startsWith(tab.path)
  );
  return match?.key ?? "";
});

const overview = ref<ExecutionOverview | null>(null);

async function loadOverview() {
  if (!hasProject.value) {
    overview.value = null;
    return;
  }
  try {
    overview.value = await getExecutionOverview(projectId.value);
  } catch {
    overview.value = null;
  }
}

onMounted(loadOverview);
watch(projectId, loadOverview);

const chips = computed(() => {
  const data = overview.value;
  if (!data) return [];
  return [
    {
      key: "active",
      label: t("s05.execution.overview.active"),
      value: String(data.active_run_count),
      tone: data.active_run_count > 0 ? "info" : "neutral"
    },
    {
      key: "waiting",
      label: t("s05.execution.overview.waiting"),
      value: String(data.waiting_count),
      tone: data.waiting_count > 0 ? "warning" : "neutral"
    },
    {
      key: "awaitingReview",
      label: t("s05.execution.overview.awaitingReview"),
      value: String(data.awaiting_review_count),
      tone: data.awaiting_review_count > 0 ? "warning" : "neutral"
    },
    {
      key: "failed",
      label: t("s05.execution.overview.failed"),
      value: String(data.failed_run_count),
      tone: data.failed_run_count > 0 ? "danger" : "neutral"
    },
    {
      key: "successRate",
      label: t("s05.execution.overview.successRate"),
      value: rateToPercent(data.success_rate),
      tone: "neutral"
    },
    {
      key: "avgDuration",
      label: t("s05.execution.overview.avgDuration"),
      value: secondsToText(data.average_duration_seconds),
      tone: "neutral"
    },
    {
      key: "devices",
      label: t("s05.execution.overview.devices"),
      value: `${data.device_ready_count}/${data.device_total_count}`,
      tone: "neutral"
    }
  ].filter(chip => chip.value.length > 0);
});
</script>

<template>
  <div class="exec-workspace">
    <nav class="exec-tabs" :aria-label="t('s05.execution.module')">
      <RouterLink
        v-for="tab in EXECUTION_TABS"
        :key="tab.key"
        :to="tab.path"
        class="exec-tabs__tab"
        :class="{ 'is-active': activeKey === tab.key }"
        :aria-current="activeKey === tab.key ? 'page' : undefined"
        :data-testid="`exec-tab-${tab.key}`"
      >
        {{ t(tab.labelKey) }}
      </RouterLink>
    </nav>

    <section
      v-if="chips.length > 0"
      class="exec-overview"
      :aria-label="t('s05.execution.overview.title')"
      data-testid="exec-overview"
    >
      <span
        v-for="chip in chips"
        :key="chip.key"
        class="exec-overview__chip"
        :class="`is-${chip.tone}`"
        :data-testid="`exec-overview-${chip.key}`"
      >
        <span class="exec-overview__label">{{ chip.label }}</span>
        <span class="exec-overview__value">{{ chip.value }}</span>
      </span>
    </section>

    <slot />
  </div>
</template>

<style scoped>
.exec-workspace {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.exec-tabs {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.exec-tabs__tab {
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

.exec-tabs__tab:hover {
  background-color: var(--surface-base);
  color: var(--ws-text-secondary);
}

.exec-tabs__tab:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
  border-radius: var(--radius-xs);
}

.exec-tabs__tab.is-active {
  color: var(--ws-text-primary);
}

.exec-tabs__tab.is-active::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--ws-accent);
  content: "";
}

.exec-overview {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-5);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-lg);
}

.exec-overview__chip {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.exec-overview__label {
  color: var(--ws-text-secondary);
}

.exec-overview__value {
  font-variant-numeric: tabular-nums;
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.exec-overview__chip.is-info .exec-overview__value {
  color: var(--ws-color-info-text);
}

.exec-overview__chip.is-warning .exec-overview__value {
  color: var(--ws-color-warning-text);
}

.exec-overview__chip.is-danger .exec-overview__value {
  color: var(--ws-color-danger-text);
}

.exec-overview__chip.is-success .exec-overview__value {
  color: var(--ws-color-success-text);
}
</style>
