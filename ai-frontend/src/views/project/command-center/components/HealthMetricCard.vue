<script setup lang="ts">
import Badge from "@/components/ui/badge/Badge.vue";
import ProgressIndicator from "@/components/ui/progress-indicator/ProgressIndicator.vue";
import type { HealthMetric } from "../project-health";

/**
 * 项目健康度的单张指标卡（Command Center 页内私有布局件）。
 *
 * 结构（Pixso「Project Command Center」3:7294 系）：
 *   标签行（标签 + 可选警示 chip）→ 主值 + 后缀 → 可选进度条 → 口径脚注。
 * 样式约束：进度条用公共 ProgressIndicator（accent）；chip 用公共 Badge 语义变体；
 * 颜色/圆角/间距全走 token。文案由调用方翻译后传入。
 */
defineProps<HealthMetric>();

const hasProgress = (progress: number | null | undefined) =>
  progress !== null && progress !== undefined;
</script>

<template>
  <div class="pm-health-metric">
    <div class="pm-health-metric__head">
      <span class="pm-health-metric__label">{{ label }}</span>
      <Badge v-if="chip" :variant="chip.tone" size="sm">{{ chip.label }}</Badge>
    </div>
    <span class="pm-health-metric__value-row">
      <span class="pm-health-metric__value">{{ value }}</span>
      <span v-if="suffix" class="pm-health-metric__suffix">{{ suffix }}</span>
    </span>
    <ProgressIndicator
      v-if="hasProgress(progress)"
      class="pm-health-metric__progress"
      :value="progress ?? 0"
      variant="accent"
      size="md"
    />
    <span v-if="hint" class="pm-health-metric__hint">{{ hint }}</span>
  </div>
</template>

<style scoped>
.pm-health-metric {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  padding: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}

.pm-health-metric__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  min-width: 0;
}

.pm-health-metric__label {
  min-width: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--text-secondary);
}

.pm-health-metric__value-row {
  display: flex;
  align-items: baseline;
  gap: var(--space-1);
}

.pm-health-metric__value {
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h3);
  font-variant-numeric: tabular-nums;
  color: var(--text-primary);
}

.pm-health-metric__suffix {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.pm-health-metric__progress {
  display: block;
}

.pm-health-metric__hint {
  font-size: var(--font-size-nano);
  line-height: 1.4;
  color: var(--text-disabled);
}
</style>
