<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { Badge } from "@/components/ui/badge";
import type { BadgeVariant } from "@/components/ui/badge/Badge.vue";

/**
 * PA-06 执行状态徽标（画布组件集 PA-06 / Execution state，7 态）。
 *
 * planned / executing / waiting / blocked / verifying / completed / failed
 * → Badge 语义色。任务域 13 态等非画布值回退 neutral（不编造语义）。
 */
const props = defineProps<{
  status: string;
}>();

const { t } = useI18n();

const KNOWN = [
  "planned",
  "executing",
  "waiting",
  "blocked",
  "verifying",
  "completed",
  "failed"
] as const;

/**
 * 任务域 13 态等后端状态 → 画布 7 态归一（显示层语义，不改状态机）：
 * draft/planned→planned · ready/running/in_progress→executing ·
 * reviewing/verifying→verifying；其余原样。
 */
const normalized = computed<(typeof KNOWN)[number] | string>(() => {
  const raw = props.status;
  if ((KNOWN as readonly string[]).includes(raw)) return raw;
  if (raw === "draft") return "planned";
  if (raw === "ready" || raw === "running" || raw === "in_progress") {
    return "executing";
  }
  if (raw === "reviewing") return "verifying";
  return raw;
});

const variant = computed<BadgeVariant>(() => {
  switch (normalized.value) {
    case "executing":
    case "verifying":
      return "accent";
    case "waiting":
      return "warning";
    case "blocked":
    case "failed":
      return "danger";
    case "completed":
      return "success";
    default:
      return "neutral";
  }
});

const label = computed(() => {
  const key = `pa06.executionState.${normalized.value}`;
  const text = t(key);
  // 未收录状态如实回显原值（不编造语义映射）
  return text === key ? props.status : text;
});
</script>

<template>
  <Badge :variant="variant" data-testid="execution-state-badge">{{
    label
  }}</Badge>
</template>
