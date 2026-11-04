<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";

import { useRelativeTime } from "../useRelativeTime";

/**
 * 我的项目卡片。
 *
 * 设计约束（高保真稿 §4.3）：只给 6 个字段（名称/角色/状态/描述/我的待办/最近活跃），
 * 不复制项目总控台的统计（任务数、需求数、Agent 数一律不展示）。
 */
interface Props {
  name: string;
  code: string;
  description: string | null;
  status: string;
  /** 当前用户在项目中的角色；未知为 null */
  role: string | null;
  pendingCount: number;
  lastActivityAt: string | null;
  /** 存在逾期货项 */
  attention: boolean;
}

const props = defineProps<Props>();

const emit = defineEmits<{ (e: "open"): void }>();

const { t } = useI18n();
const { relative } = useRelativeTime();

const statusVariant = computed(() => {
  if (props.status === "archived") return "neutral";
  if (props.status === "active") return "success";
  if (props.status === "planning") return "info";
  return "neutral";
});
</script>

<template>
  <button type="button" class="pm-project" @click="emit('open')">
    <span class="pm-project__top">
      <span class="pm-project__name">{{ name }}</span>
      <span v-if="role" class="pm-project__role">{{ role }}</span>
      <span class="pm-project__status">
        <Badge size="sm" :variant="statusVariant">{{ status }}</Badge>
      </span>
    </span>
    <span v-if="description" class="pm-project__desc">{{ description }}</span>
    <span class="pm-project__meta">
      <span
        v-if="attention"
        class="pm-project__attention"
        :title="t('pm.personalHome.projects.attentionTitle')"
      />
      <span>
        <b>{{ pendingCount }}</b> {{ t("pm.personalHome.projects.pending") }}
      </span>
      <span class="pm-project__sep" aria-hidden="true">·</span>
      <span>{{ relative(lastActivityAt) }}</span>
    </span>
  </button>
</template>

<style scoped>
.pm-project {
  display: block;
  width: 100%;
  text-align: left;
  padding: var(--space-3) var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
  transition: background var(--duration-fast) var(--ease-standard);
}

.pm-project:hover {
  background: var(--ws-hover-veil);
}

.pm-project:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.pm-project__top {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.pm-project__name {
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-project__role {
  flex: none;
  height: var(--size-badge-sm);
  display: inline-flex;
  align-items: center;
  padding: 0 5px;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

.pm-project__status {
  margin-left: auto;
  flex: none;
}

.pm-project__desc {
  display: block;
  margin-top: 4px;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-project__meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-2);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  flex-wrap: wrap;
}

.pm-project__meta b {
  font-weight: var(--weight-medium);
  font-variant-numeric: tabular-nums;
}

.pm-project__attention {
  width: 6px;
  height: 6px;
  border-radius: var(--radius-full);
  background: var(--ws-color-warning);
  flex: none;
}

.pm-project__sep {
  color: var(--ws-text-disabled);
}
</style>
