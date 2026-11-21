<script setup lang="ts">
import { useI18n } from "vue-i18n";

import { Badge } from "@/components/ui/badge";
import type { BadgeVariant } from "@/components/ui/badge/Badge.vue";

/**
 * PA-06 注意级徽标（画布组件集 PA-06 / Attention level，3 级）：
 *   informational / attention（Attention required）/ human_decision。
 */
const props = defineProps<{
  level: "informational" | "attention" | "human_decision";
  /** 徽标文本（如「2 human decisions」「AI-PM coordinating」），缺省用级别名 */
  label?: string;
}>();

const { t } = useI18n();

const VARIANT: Record<
  "informational" | "attention" | "human_decision",
  BadgeVariant
> = {
  informational: "neutral",
  attention: "warning",
  human_decision: "accent"
};
</script>

<template>
  <Badge :variant="VARIANT[props.level]" data-testid="attention-badge">
    {{ props.label ?? t(`pa06.attention.${props.level}`) }}
  </Badge>
</template>
