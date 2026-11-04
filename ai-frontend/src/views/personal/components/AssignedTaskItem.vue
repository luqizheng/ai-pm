<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";

import type { DueInfo } from "@/api/personal-home";

/**
 * Assigned Tasks 行（首页 Inbox 下分区）。
 *
 * 设计约束（高保真稿 §4.2）：普通分配任务与 Human Actions 分区呈现，
 * 单行更紧凑、无紧急度轨，优先级用等宽大写标签表达。
 */
interface Props {
  title: string;
  subject: string;
  projectName: string;
  status: string;
  priority: string;
  due: DueInfo;
}

const props = defineProps<Props>();

const emit = defineEmits<{ (e: "open"): void }>();

const { t, locale } = useI18n();

const isHigh = computed(() => props.priority === "high");

const dueLabel = computed(() => {
  const { state, daysUntilDue, dueAt } = props.due;
  if (state === "overdue" && daysUntilDue !== null) {
    return t("pm.personalHome.due.overdue", { n: Math.abs(daysUntilDue) });
  }
  if (state === "soon" && daysUntilDue === 0) {
    return t("pm.personalHome.due.dueToday");
  }
  if (dueAt) {
    const date = new Intl.DateTimeFormat(locale.value, {
      month: "short",
      day: "numeric"
    }).format(new Date(dueAt));
    return t("pm.personalHome.due.dueOn", { date });
  }
  return t("pm.personalHome.due.noDueDate");
});

const dueTone = computed(() => {
  if (props.due.state === "overdue") return "danger";
  if (props.due.state === "soon") return "warning";
  return "muted";
});
</script>

<template>
  <button type="button" class="pm-task" @click="emit('open')">
    <span class="pm-task__priority" :class="{ 'is-high': isHigh }">
      {{ priority }}
    </span>
    <span class="pm-task__body">
      <span class="pm-task__title">{{ title }}</span>
      <span class="pm-task__meta">
        <span>{{ projectName }}</span>
        <span class="pm-task__sep" aria-hidden="true">·</span>
        <span>{{ subject }}</span>
      </span>
    </span>
    <span class="pm-task__right">
      <Badge size="sm">{{ status }}</Badge>
      <span class="pm-task__due" :data-tone="dueTone">{{ dueLabel }}</span>
    </span>
  </button>
</template>

<style scoped>
.pm-task {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  text-align: left;
  padding: 9px var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
  transition: background var(--duration-fast) var(--ease-standard);
}

.pm-task:hover {
  background: var(--ws-hover-veil);
}

.pm-task:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.pm-task__priority {
  width: 44px;
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--ws-text-secondary);
}

.pm-task__priority.is-high {
  color: var(--ws-color-warning-text);
}

.pm-task__body {
  flex: 1;
  min-width: 0;
}

.pm-task__title {
  display: block;
  font-size: var(--font-size-body);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-task__meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 1px;
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.pm-task__sep {
  color: var(--ws-text-disabled);
}

.pm-task__right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex: none;
}

.pm-task__due {
  font-size: var(--font-size-caption);
  white-space: nowrap;
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.pm-task__due[data-tone="danger"] {
  color: var(--ws-color-danger-text);
}

.pm-task__due[data-tone="warning"] {
  color: var(--ws-color-warning-text);
}
</style>
