<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";
import ListItem from "@/components/ui/list-item/ListItem.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";

import type { BadgeVariant } from "@/components/ui/badge/Badge.vue";
import { useRelativeTime } from "../useRelativeTime";

/**
 * 我的项目行 —— 组合公共 ListItem。
 *
 * 设计约束（高保真稿 §4.3）：只给 6 个字段（名称/角色/状态/描述/我的待办/最近活跃），
 * 不复制项目总控台的统计（任务数、需求数、Agent 数一律不展示）。
 * 角色与状态走公共 Badge，关注指示走公共 StatusIndicator。
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
  /** 存在逾期人工事项 */
  attention: boolean;
}

const props = defineProps<Props>();

const emit = defineEmits<{ (e: "open"): void }>();

const { t } = useI18n();
const { relative } = useRelativeTime();

const statusVariant = computed<BadgeVariant>(() => {
  if (props.status === "archived") return "neutral";
  if (props.status === "active") return "success";
  if (props.status === "planning") return "info";
  return "neutral";
});

const summary = computed(() => {
  const parts = [
    props.description || props.code,
    `${props.pendingCount} ${t("pm.personalHome.projects.pending")}`,
    relative(props.lastActivityAt)
  ].filter(Boolean);
  return parts.join(" · ");
});
</script>

<template>
  <ListItem
    class="pm-project"
    :title="name"
    :description="summary"
    @click="emit('open')"
  >
    <template #trailing>
      <StatusIndicator
        v-if="attention"
        status="warning"
        size="sm"
        :title="t('pm.personalHome.projects.attentionTitle')"
      />
      <Badge v-if="role" size="sm">{{ role }}</Badge>
      <Badge size="sm" :variant="statusVariant">{{ status }}</Badge>
    </template>
  </ListItem>
</template>
