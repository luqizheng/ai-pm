<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";
import ListItem from "@/components/ui/list-item/ListItem.vue";

import type { DueInfo, HumanActionKind } from "@/api/personal-home";

/**
 * Human Actions 行（首页 Inbox 上分区）—— 组合公共 ListItem。
 *
 * 设计约束（高保真稿 §4.2 / §5.1）：
 *   - 类型 chip 一律中性色；紧急度只由截止文案的颜色承载
 *     （后端当前无 due 列 → 全部「无期限」，轨/档位自动生效是后端补字段后的事）；
 *   - 每条必须能追溯到来源项目与原始业务对象（description 行）。
 */
interface Props {
  kind: HumanActionKind;
  title: string | null;
  subject: string;
  objectType: string;
  projectName: string;
  due: DueInfo;
}

const props = defineProps<Props>();

const emit = defineEmits<{ (e: "open"): void }>();

const { t, locale } = useI18n();

const kindLabel = computed(() =>
  t(`pm.personalHome.inbox.kinds.${props.kind}`)
);

const description = computed(() => {
  const object = `${props.objectType} ${props.subject}`;
  return props.projectName ? `${props.projectName} · ${object}` : object;
});

/** 右上角截止信息 */
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
    class="pm-action"
    :title="title ?? kindLabel"
    :description="description"
    @click="emit('open')"
  >
    <template #trailing>
      <Badge size="sm">{{ kindLabel }}</Badge>
      <span class="pm-action__due" :data-tone="dueTone">{{ dueLabel }}</span>
    </template>
  </ListItem>
</template>

<style scoped>
.pm-action__due {
  font-size: var(--font-size-caption);
  white-space: nowrap;
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.pm-action__due[data-tone="danger"] {
  color: var(--ws-color-danger-text);
}

.pm-action__due[data-tone="warning"] {
  color: var(--ws-color-warning-text);
}
</style>
