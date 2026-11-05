<script setup lang="ts">
import Card from "@/components/ui/card/Card.vue";
import SectionHead from "./SectionHead.vue";
import TaskStatCard from "./TaskStatCard.vue";
import type { TaskStat } from "../task-overview";

/**
 * 任务概览卡（Pixso「Project Command Center」3:7365/3:7366）。
 *
 * 组合策略（AGENTS.md §三.11）：外壳用公共 Card，头部用页内共享 SectionHead；
 * 页内只保留布局样式（Card 头部压缩为无边框紧凑行），文案由调用方翻译后传入。
 */
interface Props {
  /** 卡片标题（如「任务概览」） */
  title: string;
  /** 总数徽标文案（如「24 个任务」）；不传则不渲染 */
  totalLabel?: string;
  /** 「全部任务」入口文案 */
  viewAllLabel: string;
  /** 迷你统计卡（设计稿为三张） */
  stats: TaskStat[];
}

withDefaults(defineProps<Props>(), { totalLabel: "" });

const emit = defineEmits<{ (e: "view-all"): void }>();
</script>

<template>
  <Card class="pm-task-overview">
    <template #header>
      <SectionHead
        :title="title"
        :meta="totalLabel"
        :action-label="viewAllLabel"
        @action="emit('view-all')"
      />
    </template>
    <div class="pm-task-overview__grid">
      <TaskStatCard v-for="stat in stats" :key="stat.label" v-bind="stat" />
    </div>
  </Card>
</template>

<style scoped>
/* Card 头部压缩为设计稿的紧凑行（无边框分隔、卡片内 12px 内边距） */
.pm-task-overview :deep(.ui-card__header) {
  padding: var(--space-3) var(--space-3) 0;
  border-bottom: 0;
}

.pm-task-overview :deep(.ui-card__body) {
  padding: var(--space-2) var(--space-3) var(--space-3);
}

.pm-task-overview__grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2);
}
</style>
