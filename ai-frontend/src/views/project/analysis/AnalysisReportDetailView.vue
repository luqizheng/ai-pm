<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import PageFrame from "@/components/shell/PageFrame.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { useShellStore } from "@/stores/shell";

/**
 * Analysis 报告详情页（/project/analysis/reports/:reportId，P2-01 占位框架）。
 *
 * - P2-03 按裁决 D-2 充实为六节结构（基本信息 / KB 正文 / PC 提案 /
 *   PC 修订 / D-4 综合 / 关闭与处置）；本卡只交付路由与项目上下文守卫。
 * - 非法/跨项目 Report ID 的显式错误态（GAP-6）随 P2-03 关闭。
 * - 项目上下文单源 = shell.currentProjectId，同列表页。
 */
const { t } = useI18n();
const shell = useShellStore();
const currentProjectId = computed(() => shell.currentProjectId);
</script>

<template>
  <PageFrame :title="t('analysis.detail.title')">
    <EmptyState
      v-if="!currentProjectId"
      size="sm"
      :title="t('analysis.state.noProject')"
    />
    <EmptyState v-else size="sm" :title="t('analysis.state.detailFramework')" />
  </PageFrame>
</template>
