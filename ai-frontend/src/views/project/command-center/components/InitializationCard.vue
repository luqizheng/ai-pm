<script setup lang="ts">
import { computed } from "vue";

import Card from "@/components/ui/card/Card.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type {
  CommandCenterInitData,
  InitItemView
} from "../command-center-data";

/**
 * 项目初始化进度卡（S01-01 Initialization Overview）。
 *
 * 数据 = `/command-center` 的 `initialization_progress` 块（后端早已返回，
 * 此前前端未映射、未渲染，导致 S01-01 缺页）。
 *
 * 关键语义：`completion = not_determinable` 时**不得宣称已完成**——
 * 无项目类型/初始化要求配置就无法判定，页面显示「无法判定」而不是绿灯。
 */
const props = defineProps<{
  /** 卡标题 */
  title: string;
  /** 进度概览文案（调用方翻译，如「必需 2 / 3」） */
  progressLabel: string;
  /** 各完成度的判定文案（调用方翻译） */
  completionLabels: {
    complete: string;
    incomplete: string;
    notDeterminable: string;
  };
  /** 必需事项标记文案 */
  requiredLabel: string;
  /** 数据（null = 后端未提供该块，整卡不渲染） */
  data: CommandCenterInitData | null;
}>();

const hasData = computed(() => props.data !== null);

const completionLabel = computed(() => {
  if (!props.data) return "";
  switch (props.data.completion) {
    case "complete":
      return props.completionLabels.complete;
    case "incomplete":
      return props.completionLabels.incomplete;
    default:
      // not_determinable 与任何未知值都按「无法判定」处理——不猜完成。
      return props.completionLabels.notDeterminable;
  }
});

const completionTone = computed<"success" | "warning" | "neutral">(() => {
  if (!props.data) return "neutral";
  switch (props.data.completion) {
    case "complete":
      return "success";
    case "incomplete":
      return "warning";
    default:
      return "neutral";
  }
});

const items = computed<InitItemView[]>(() => props.data?.items ?? []);
</script>

<template>
  <Card
    v-if="hasData && data"
    class="init-card"
    data-testid="cc-initialization"
  >
    <template #header>
      <div class="init-card__head">
        <span>{{ title }}</span>
        <Badge :variant="completionTone" size="sm">
          {{ completionLabel }}
        </Badge>
        <span class="init-card__progress">{{ progressLabel }}</span>
      </div>
    </template>

    <ul class="init-list">
      <li
        v-for="item in items"
        :key="item.key"
        class="init-row"
        :data-testid="`cc-init-${item.key}`"
      >
        <StatusIndicator
          :status="
            item.tone === 'success'
              ? 'success'
              : item.tone === 'danger'
                ? 'danger'
                : item.tone === 'info'
                  ? 'info'
                  : 'neutral'
          "
          size="sm"
        />
        <span class="init-row__title">{{ item.title }}</span>
        <Badge v-if="item.required" variant="neutral" size="sm" outline>
          {{ requiredLabel }}
        </Badge>
        <span class="init-row__status">{{ item.statusLabel }}</span>
        <span v-if="item.detail" class="init-row__detail">{{
          item.detail
        }}</span>
      </li>
    </ul>
  </Card>
</template>

<style scoped>
.init-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.init-card__head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

.init-card__progress {
  margin-left: auto;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-secondary);
}

.init-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.init-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.init-row__title {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

.init-row__status {
  color: var(--ws-text-secondary);
}

.init-row__detail {
  flex-basis: 100%;
  color: var(--ws-text-secondary);
}
</style>
