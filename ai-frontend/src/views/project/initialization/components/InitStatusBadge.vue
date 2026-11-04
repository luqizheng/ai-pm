<script setup lang="ts">
import { computed, type Component } from "vue";
import { useI18n } from "vue-i18n";
import {
  Circle,
  CircleCheck,
  CircleHelp,
  LoaderCircle,
  Minus,
  TriangleAlert,
  UserRound
} from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import type { BadgeVariant } from "@/components/ui/badge";
import type { StageStatus } from "@/api/initialization-overview";

/**
 * 阶段状态徽标。
 *
 * 规则（设计系统 §1.1 + S01-01 稿 §4.1）：
 *   - 状态**必须**同时带文字与图标，颜色不得单独承载语义；
 *   - 颜色只表达状态语义，不表达事项类型；
 *   - 「等待你确认」与「存在阻塞」必须可区分——前者是等你动作，后者是外部条件阻塞。
 */

interface Props {
  status: StageStatus;
  size?: "sm" | "md" | "lg";
}

const props = withDefaults(defineProps<Props>(), { size: "md" });

const { t } = useI18n();

const META: Record<StageStatus, { variant: BadgeVariant; icon: Component }> = {
  not_started: { variant: "neutral", icon: Circle },
  in_progress: { variant: "info", icon: LoaderCircle },
  waiting_for_human: { variant: "warning", icon: UserRound },
  blocked: { variant: "danger", icon: TriangleAlert },
  completed: { variant: "success", icon: CircleCheck },
  not_applicable: { variant: "neutral", icon: Minus },
  undetermined: { variant: "neutral", icon: CircleHelp }
};

const meta = computed(() => META[props.status] ?? META.undetermined);
const label = computed(() =>
  t(`s01.initialization-overview.status.${props.status}`, props.status)
);
</script>

<template>
  <Badge :variant="meta.variant" :size="props.size">
    <component :is="meta.icon" :size="12" aria-hidden="true" />
    <span>{{ label }}</span>
  </Badge>
</template>
