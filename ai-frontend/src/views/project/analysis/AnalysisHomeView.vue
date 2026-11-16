<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import PageFrame from "@/components/shell/PageFrame.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { useShellStore } from "@/stores/shell";

/**
 * Analysis 独立首页（/project/analysis，P2-01）——报告列表页框架。
 *
 * - 项目上下文单源 = shell.currentProjectId：无项目上下文时空态，
 *   不展示其他项目数据（复用现有项目切换守卫，不另建上下文）。
 * - 报告列表随 P2-02 从 AnalysisReportsPanel 迁入；本卡交付路由 + 框架 +
 *   「无项目」态（加载/空/错误态随列表迁移落地，见 analysis.state.framework）。
 */
const { t } = useI18n();
const shell = useShellStore();
const currentProjectId = computed(() => shell.currentProjectId);
</script>

<template>
  <PageFrame
    :title="t('analysis.page.title')"
    :description="t('analysis.page.description')"
  >
    <EmptyState
      v-if="!currentProjectId"
      size="sm"
      :title="t('analysis.state.noProject')"
    />
    <EmptyState v-else size="sm" :title="t('analysis.state.framework')" />
  </PageFrame>
</template>
