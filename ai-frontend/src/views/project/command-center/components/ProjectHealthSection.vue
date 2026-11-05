<script setup lang="ts">
import SectionHead from "./SectionHead.vue";
import HealthMetricCard from "./HealthMetricCard.vue";
import type { HealthMetric } from "../project-health";

/**
 * 项目健康度区（Pixso「Project Command Center」3:7283）。
 *
 * 结构：SectionHead（标题 + 「更新于 HH:mm」chip + 「查看指标定义」入口）
 * → 五张指标卡网格（进度/需求/任务带进度条；风险与阻塞、待人工决策带警示 chip）。
 * 无外卡壳——设计稿为画布上的裸区块。文案由调用方翻译后传入。
 */
interface Props {
  /** 区块标题（如「项目健康度」） */
  title: string;
  /** 弱化元信息 chip（如「更新于 09:42」）；不传不渲染 */
  meta?: string;
  /** 右侧入口文案（如「查看指标定义」）；不传不渲染 */
  actionLabel?: string;
  /** 指标卡（设计稿五张） */
  metrics: HealthMetric[];
}

withDefaults(defineProps<Props>(), { meta: "", actionLabel: "" });

const emit = defineEmits<{ (e: "action"): void }>();
</script>

<template>
  <section class="pm-health">
    <SectionHead
      :title="title"
      :meta="meta"
      :action-label="actionLabel"
      @action="emit('action')"
    />
    <div class="pm-health__grid">
      <HealthMetricCard v-for="metric in metrics" :key="metric.label" v-bind="metric" />
    </div>
  </section>
</template>

<style scoped>
.pm-health {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.pm-health__grid {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: var(--space-2);
}
</style>
