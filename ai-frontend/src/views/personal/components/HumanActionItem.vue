<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";

import type { DueInfo, HumanActionKind } from "@/api/personal-home";

/**
 * Human Actions 行（首页 Inbox 上分区）。
 *
 * 设计约束（高保真稿 §4.2 / §5.1）：
 *   - 类型 chip 一律中性色；颜色只由左侧 2px 紧急度轨承载；
 *   - 每条必须能追溯到来源项目与原始业务对象；
 *   - 无截止时间显式写「无期限」，不虚构期限。
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
  <button type="button" class="pm-action" @click="emit('open')">
    <span class="pm-action__rail" :data-tone="dueTone" aria-hidden="true" />
    <span class="pm-action__body">
      <span class="pm-action__title">
        <Badge size="sm">{{ kindLabel }}</Badge>
        <span class="pm-action__text">{{ title ?? kindLabel }}</span>
      </span>
      <span class="pm-action__meta">
        <span class="pm-action__project">{{ projectName }}</span>
        <span class="pm-action__sep" aria-hidden="true">·</span>
        <span class="pm-action__object"> {{ objectType }} {{ subject }} </span>
      </span>
    </span>
    <span class="pm-action__due" :data-tone="dueTone">{{ dueLabel }}</span>
  </button>
</template>

<style scoped>
.pm-action {
  display: grid;
  grid-template-columns: var(--size-indicator) minmax(0, 1fr) auto;
  gap: var(--space-3);
  align-items: center;
  width: 100%;
  text-align: left;
  padding: var(--space-3) var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
  transition: background var(--duration-fast) var(--ease-standard);
}

.pm-action:hover {
  background: var(--ws-hover-veil);
}

.pm-action:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.pm-action__rail {
  align-self: stretch;
  border-radius: var(--radius-nav-indicator);
  background: transparent;
}

.pm-action__rail[data-tone="danger"] {
  background: var(--ws-color-danger);
}

.pm-action__rail[data-tone="warning"] {
  background: var(--ws-color-warning);
}

.pm-action__body {
  min-width: 0;
}

.pm-action__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.pm-action__text {
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-action__meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 3px;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
  min-width: 0;
  flex-wrap: wrap;
}

.pm-action__project {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-action__sep {
  color: var(--ws-text-disabled);
}

.pm-action__object {
  white-space: nowrap;
}

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
