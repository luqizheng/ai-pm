<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import SettingsAppFrame from "@/views/settings/SettingsAppFrame.vue";
import { useProjectSettingsFrame } from "../shared";

/**
 * Project Setting · AI Team 各子页共用壳。
 *
 * 组织方式（UI-1 裁决记录）：外层沿用设置域侧栏的单条「AI 配置」入口，
 * 五个子页（Roles / Overview / Model Policies / Execution Monitor /
 * Execution Policies）走 **子路由 + 页内页签条**——与设置域「一节一路由」
 * 的既有组织同构，五个子页各自独立 URL（可深链、携带 ?project_id= 上下文），
 * 后续 Overview / Monitor / ExecutionPolicies 批次只增路由不动已落地页。
 *
 * 页签条是路由链接（非 ui/tabs 组件）：子页是独立路由而非同页切换，
 * RouterLink 才能保留浏览器前进/后退与深链语义。
 */
const props = defineProps<{
  /** 子页描述（已翻译；空省略） */
  description?: string;
}>();

const { t } = useI18n();
const route = useRoute();
const settingsFrame = useProjectSettingsFrame();

/** 五子页固定顺序 = 落地序（U-2=A：Roles → Overview → Model Policies → Monitor → ExecutionPolicies） */
const AI_TABS = [
  {
    key: "roles",
    path: "/project/settings/ai/roles",
    labelKey: "aiTeam.tabs.roles"
  },
  {
    key: "overview",
    path: "/project/settings/ai/overview",
    labelKey: "aiTeam.tabs.overview"
  },
  {
    key: "modelPolicies",
    path: "/project/settings/ai/model-policies",
    labelKey: "aiTeam.tabs.modelPolicies"
  },
  {
    key: "monitor",
    path: "/project/settings/ai/monitor",
    labelKey: "aiTeam.tabs.monitor"
  },
  {
    key: "executionPolicies",
    path: "/project/settings/ai/execution-policies",
    labelKey: "aiTeam.tabs.executionPolicies"
  }
] as const;

const tabs = computed(() =>
  AI_TABS.map(tab => ({
    ...tab,
    label: t(tab.labelKey),
    to: settingsFrame.withProject(tab.path)
  }))
);

const activeKey = computed(
  () => AI_TABS.find(tab => tab.path === route.path)?.key ?? ""
);

const sectionTitle = computed(() => t("settings.nav.projectAi"));
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="settingsFrame.breadcrumb(sectionTitle)"
    :title="sectionTitle"
    :description="props.description"
    :scope-label="settingsFrame.scopeLabel.value"
    :entries="settingsFrame.navEntries.value"
    :icons="settingsFrame.navIcons"
  >
    <template v-if="$slots.actions" #actions>
      <slot name="actions" />
    </template>
    <nav class="ai-tabs" data-testid="ai-tabs" :aria-label="sectionTitle">
      <RouterLink
        v-for="tab in tabs"
        :key="tab.key"
        :to="tab.to"
        class="ai-tabs__tab"
        :class="{ 'is-active': activeKey === tab.key }"
        :aria-current="activeKey === tab.key ? 'page' : undefined"
        :data-testid="`ai-tab-${tab.key}`"
      >
        {{ tab.label }}
      </RouterLink>
    </nav>

    <slot />
  </SettingsAppFrame>
</template>

<style scoped>
/* 页签条沿用 DS/Navigation/Tab 口径（透明底行 + active 底缘 2px accent 指示条），
 * 但实现为 RouterLink（子页是独立路由，需保留深链与前进/后退语义）。 */
.ai-tabs {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-tabs__tab {
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

.ai-tabs__tab:hover {
  background-color: var(--surface-base);
  color: var(--ws-text-secondary);
}

.ai-tabs__tab:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
  border-radius: var(--radius-xs);
}

.ai-tabs__tab.is-active {
  color: var(--ws-text-primary);
}

.ai-tabs__tab.is-active::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--ws-accent);
  content: "";
}
</style>
