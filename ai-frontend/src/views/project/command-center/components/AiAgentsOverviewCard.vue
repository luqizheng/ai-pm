<script setup lang="ts">
import { Bot } from "lucide-vue-next";
import Card from "@/components/ui/card/Card.vue";
import ProgressIndicator from "@/components/ui/progress-indicator/ProgressIndicator.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import SectionHead from "./SectionHead.vue";
import type { AiAgentsColumnLabels, AgentMetric, AgentRow } from "../ai-agents";

/**
 * AI Agents 概览卡（Pixso「Project Command Center」3:7475）。
 *
 * 结构：头部（SectionHead）→ 三张紧凑指标 tile → 分隔线 → 列头带 → Agent 行
 * （bot 图标+名称+角色 ｜ 当前工作 ｜ 状态语义点 ｜ 进度条 ｜ 时长）。
 * 样式约束：进度条/语义点用公共 ProgressIndicator/StatusIndicator，不自造；
 * 行状态不做软底 chip（深色主题下点+文字更轻）；
 * 已完成的行进度条走 success 变体，其余 accent。文案由调用方翻译后传入。
 */
interface Props {
  /** 卡片标题（如「AI Agents 概览」） */
  title: string;
  /** 弱化元信息 chip（如「独立于任务状态」）；不传不渲染 */
  meta?: string;
  /** 右侧入口文案（如「AI R&D」）；不传不渲染 */
  actionLabel?: string;
  /** 紧凑指标（设计稿三张） */
  metrics: AgentMetric[];
  /** 列头文案 */
  columnLabels: AiAgentsColumnLabels;
  /** Agent 行（设计稿四行） */
  rows: AgentRow[];
}

withDefaults(defineProps<Props>(), { meta: "", actionLabel: "" });

const emit = defineEmits<{ (e: "action"): void }>();

const progressVariant = (progress: number) =>
  progress >= 100 ? "success" : "accent";
const hasProgress = (progress: number | null | undefined) =>
  progress !== null && progress !== undefined;
</script>

<template>
  <Card class="pm-agents">
    <template #header>
      <SectionHead
        :title="title"
        :meta="meta"
        :action-label="actionLabel"
        @action="emit('action')"
      />
    </template>

    <div class="pm-agents__metrics">
      <div v-for="metric in metrics" :key="metric.label" class="pm-agents__metric">
        <span class="pm-agents__metric-label">{{ metric.label }}</span>
        <span class="pm-agents__metric-value">
          {{ metric.primary }}
          <span v-if="metric.secondary" class="pm-agents__metric-secondary">
            {{ metric.secondary }}
          </span>
        </span>
      </div>
    </div>

    <div class="pm-agents__divider" role="presentation" />

    <div class="pm-agents__row pm-agents__row--head" aria-hidden="true">
      <span class="pm-agents__col-head">{{ columnLabels.agent }}</span>
      <span class="pm-agents__col-head">{{ columnLabels.work }}</span>
      <span class="pm-agents__col-head">{{ columnLabels.status }}</span>
      <span class="pm-agents__col-head">{{ columnLabels.progress }}</span>
      <span class="pm-agents__col-head pm-agents__col-head--end">
        {{ columnLabels.duration }}
      </span>
    </div>

    <ul class="pm-agents__rows">
      <li v-for="row in rows" :key="row.name" class="pm-agents__row">
        <span class="pm-agents__agent">
          <Bot class="pm-agents__bot" aria-hidden="true" />
          <span class="pm-agents__name">{{ row.name }}</span>
          <span class="pm-agents__role">{{ row.role }}</span>
        </span>
        <span class="pm-agents__work" :title="row.work">{{ row.work }}</span>
        <StatusIndicator
          class="pm-agents__status"
          :status="row.statusTone"
          size="sm"
          :label="row.statusLabel"
        />
        <span class="pm-agents__progress">
          <ProgressIndicator
            v-if="hasProgress(row.progress)"
            :value="row.progress ?? 0"
            :variant="progressVariant(row.progress ?? 0)"
            size="md"
          />
        </span>
        <span class="pm-agents__duration">{{ row.duration }}</span>
      </li>
    </ul>
  </Card>
</template>

<style scoped>
.pm-agents :deep(.ui-card__header) {
  padding: var(--space-3) var(--space-3) 0;
  border-bottom: 0;
}

.pm-agents :deep(.ui-card__body) {
  display: flex;
  flex-direction: column;
  padding: 0;
}

/* 紧凑指标行（设计稿：三张 58h 小 tile，标签弱化 + 混排值） */
.pm-agents__metrics {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3) var(--space-3);
}

.pm-agents__metric {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  padding: var(--space-2) var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}

.pm-agents__metric-label {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.pm-agents__metric-value {
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h4);
  font-variant-numeric: tabular-nums;
  color: var(--text-primary);
}

.pm-agents__metric-secondary {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-regular);
  color: var(--text-disabled);
}

.pm-agents__divider {
  flex: none;
  height: var(--border-1);
  background: var(--border-default);
}

/* 列头带与行共用五列网格：Agent ｜ 当前工作 ｜ 状态 ｜ 进度 ｜ 时长 */
.pm-agents__row {
  display: grid;
  grid-template-columns:
    minmax(0, 1.5fr) minmax(0, 1.2fr) auto 72px 48px;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
}

.pm-agents__row--head {
  padding-block: var(--space-1);
  border-top: var(--border-1) solid var(--border-subtle);
  border-bottom: var(--border-1) solid var(--border-subtle);
}

.pm-agents__col-head {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-disabled);
}

.pm-agents__col-head--end {
  text-align: right;
}

.pm-agents__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}

.pm-agents__rows .pm-agents__row + .pm-agents__row {
  border-top: var(--border-1) solid var(--border-subtle);
}

.pm-agents__agent {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.pm-agents__bot {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  flex: none;
  color: var(--accent);
}

.pm-agents__name {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
  white-space: nowrap;
}

.pm-agents__role {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  white-space: nowrap;
}

.pm-agents__work {
  overflow: hidden;
  min-width: 0;
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-agents__status {
  flex: none;
}

.pm-agents__progress {
  display: block;
  min-width: 0;
}

.pm-agents__duration {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  text-align: right;
  font-variant-numeric: tabular-nums;
}
</style>
