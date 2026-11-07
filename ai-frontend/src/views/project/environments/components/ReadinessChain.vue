<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import Badge from "@/components/ui/badge/Badge.vue";
import type { ChainStepState, EnvChainStep } from "../domain";

/**
 * ReadinessChain —— 设备就绪阶段链（线稿：Installed → Connected →
 * Authenticated → 终态 四枚 chip）。
 *
 * 纯展示派生（domain.readinessChainSteps），颜色随步骤状态：
 * pass=success / warning=warning / fail=danger / pending・unknown=neutral。
 */
const props = defineProps<{
  steps: EnvChainStep[];
  /** 紧凑模式（卡片内）；详情页可用默认尺寸 */
  compact?: boolean;
}>();

const { t } = useI18n();

type ChainBadgeVariant = "success" | "warning" | "danger" | "neutral";

const VARIANT_BY_STATE: Record<ChainStepState, ChainBadgeVariant> = {
  pass: "success",
  warning: "warning",
  fail: "danger",
  pending: "neutral",
  unknown: "neutral"
};

const badges = computed(() =>
  props.steps.map(step => ({
    key: step.key,
    label: t(step.labelKey),
    variant: VARIANT_BY_STATE[step.state]
  }))
);
</script>

<template>
  <ul
    class="env-chain"
    :class="{ 'env-chain--compact': props.compact }"
    data-testid="env-chain"
  >
    <li v-for="badge in badges" :key="badge.key" class="env-chain__item">
      <Badge :variant="badge.variant" size="sm">{{ badge.label }}</Badge>
    </li>
  </ul>
</template>

<style scoped>
.env-chain {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  padding: 0;
  margin: 0;
  list-style: none;
}

@media (prefers-reduced-motion: reduce) {
  .env-chain,
  .env-chain__item {
    transition: none;
  }
}
</style>
