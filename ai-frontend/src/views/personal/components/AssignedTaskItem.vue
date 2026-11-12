<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge, { type BadgeVariant } from "@/components/ui/badge/Badge.vue";
import ListItem from "@/components/ui/list-item/ListItem.vue";

import type { DueInfo } from "@/api/personal-home";

/**
 * Assigned Tasks 行（首页 Inbox 下分区）—— 组合公共 ListItem。
 *
 * 设计约束（高保真稿 §4.2）：普通分配任务与 Human Actions 分区呈现，
 * 优先级用等宽大写标签表达（high 才着色），状态走公共 Badge。
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

const statusVariant = computed<BadgeVariant>(() => {
  if (props.status === "completed") return "success";
  if (props.status === "in_progress") return "info";
  return "neutral";
});

const description = computed(() => {
  const object = props.subject;
  return props.projectName ? `${props.projectName} · ${object}` : object;
});

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
  <ListItem
    class="pm-task"
    :title="title"
    :description="description"
    @click="emit('open')"
  >
    <template #trailing>
      <span v-if="priority === 'high'" class="pm-task__priority">{{
        priority
      }}</span>
      <Badge :variant="statusVariant" size="sm">{{ status }}</Badge>
      <span class="pm-task__due" :data-tone="dueTone">{{ dueLabel }}</span>
    </template>
  </ListItem>
</template>

<style scoped>
.pm-task__priority {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  letter-spacing: var(--tracking-caption);
  text-transform: uppercase;
  color: var(--ws-color-warning-text);
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
