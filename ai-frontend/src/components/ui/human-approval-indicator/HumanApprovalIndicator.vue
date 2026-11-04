<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/AI/HumanApprovalIndicator" node=13:34834 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { CircleCheck, CircleDashed, CircleX, Clock } from "lucide-vue-next";
import { cn } from "@/lib/cn";

/**
 * HumanApprovalIndicator —— Figma DS/AI/HumanApprovalIndicator（13:34834）。
 * 人工确认指示 chip：状态图标 + 标签（+ 可选操作人）。
 * ⚠ AGENTS.md 硬规则落点：**人工确认提示用次文字色**（非 accent），
 * 与 AI 建议（accent）视觉可辨 —— AI 呈现 ≠ 人工已批准。
 */

export type ApprovalSize = "sm" | "md" | "lg";
export type ApprovalState =
  | "pending"
  | "approved"
  | "rejected"
  | "not-required";

import type { Component } from "vue";

interface Props {
  state?: ApprovalState;
  size?: ApprovalSize;
  label?: string;
  /** 操作人（如 Lead: Alex） */
  actor?: string;
}

const props = withDefaults(defineProps<Props>(), {
  state: "pending",
  size: "md",
  label: "",
  actor: ""
});

const STATE_ICONS: Record<ApprovalState, Component> = {
  pending: Clock,
  approved: CircleCheck,
  rejected: CircleX,
  "not-required": CircleDashed
};

const icon = computed(() => STATE_ICONS[props.state]);
const hasLabel = computed(() => props.label.length > 0);
const hasActor = computed(() => props.actor.length > 0);
</script>

<template>
  <span
    :class="cn('ui-approval', `ui-approval--${size}`, `is-${state}`)"
    role="status"
  >
    <component :is="icon" class="ui-approval__icon" aria-hidden="true" />
    <span v-if="hasLabel" class="ui-approval__label">{{ label }}</span>
    <span v-if="hasActor" class="ui-approval__actor">{{ actor }}</span>
  </span>
</template>

<style scoped>
/* chip：padding 1px 5px / radius 4 / elevated 底 + border 描边（Figma 键盘 chip 同构） */
.ui-approval {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 1px var(--space-1);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
}

.ui-approval--sm .ui-approval__icon,
.ui-approval--sm .ui-approval__label {
  font-size: var(--font-size-nano);
}

.ui-approval__icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  flex: none;
}

.ui-approval__label,
.ui-approval__actor {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

/* 人工确认提示 = 次文字色（非 accent，与 AI 建议可辨） */
.ui-approval__label {
  color: var(--text-secondary);
}

.ui-approval__actor {
  color: var(--text-disabled);
}

.ui-approval.is-approved .ui-approval__icon {
  color: var(--ws-color-success-text);
}

.ui-approval.is-rejected .ui-approval__icon {
  color: var(--ws-color-danger);
}

.ui-approval.is-pending .ui-approval__icon,
.ui-approval.is-not-required .ui-approval__icon {
  color: var(--text-secondary);
}
</style>
