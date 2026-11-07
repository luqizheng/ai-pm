<script setup lang="ts">
import { computed } from "vue";
import type { Component } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";

import type { SettingsNavEntry } from "./settings.model";
import SettingsNavigationItem from "./SettingsNavigationItem.vue";

/**
 * SettingsSidebar —— 设置页侧栏导航（Settings Framework §5/§7）。
 *
 * 输入 = 域内条目列表（调用方先按权限过滤 hidden，§9 权限感知）；
 * 标签在组件内统一取 i18n（settings.nav.<id>）；激活态由当前路由派生。
 */
const props = defineProps<{
  /** 域标题（已翻译，如「个人账户设置」） */
  scopeLabel: string;
  /** 域内导航条目（已按权限过滤） */
  entries: SettingsNavEntry[];
  /** 图标组件（lucide；与条目一一对应可选） */
  icons?: Record<string, Component>;
  /** 无权访问整域时置灰全部条目（denied 演示态） */
  denied?: boolean;
}>();

const { t } = useI18n();
const route = useRoute();

const items = computed(() =>
  props.entries.map(entry => ({
    entry,
    label: t(`settings.nav.${entry.id}`),
    icon: props.icons?.[entry.id]
  }))
);

/**
 * 激活判定 = 精确命中或前缀命中（如 Runner 详情页
 * /project/settings/runners/:id 命中 runners 条目）。
 * 同前缀兄弟路径不会误判：/project/settings/runners-xxx 不匹配 `runners/`。
 * 条目 to 可携带查询串（项目设置带 ?project_id=），只比路径部分。
 */
function isActive(to: string): boolean {
  const path = to.split(/[?#]/)[0];
  return route.path === path || route.path.startsWith(`${path}/`);
}
</script>

<template>
  <nav class="settings-sidebar" :aria-label="scopeLabel">
    <p class="settings-sidebar__scope">{{ scopeLabel }}</p>
    <div class="settings-sidebar__list">
      <SettingsNavigationItem
        v-for="item in items"
        :key="item.entry.id"
        :label="item.label"
        :to="denied ? undefined : item.entry.to"
        :icon="item.icon"
        :active="!denied && isActive(item.entry.to)"
        :proposed="item.entry.proposed"
        :frame="item.entry.frame"
        :disabled="denied"
      />
    </div>
  </nav>
</template>

<style scoped>
.settings-sidebar {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.settings-sidebar__scope {
  margin: 0;
  padding: 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
  text-transform: uppercase;
  letter-spacing: var(--tracking-caption);
}

.settings-sidebar__list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
</style>
