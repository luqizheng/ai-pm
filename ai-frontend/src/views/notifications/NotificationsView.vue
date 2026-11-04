<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";

import {
  NotificationCenter,
  NotificationDetail
} from "@/components/global/notification";
import type { NotificationRecord } from "@/components/global/notification/types";
import PageFrame from "@/components/shell/PageFrame.vue";
import { useNotificationsStore } from "@/stores/notifications";

/**
 * 通知中心页（/notifications，顶栏下拉「查看全部通知」的落点）。
 *
 * 布局 = 统一页框（AGENTS.md §三.12，工作台型页面省略标题区）
 *        内嵌完整通知中心（分类/未读过滤 + 记录列表）+ 选中记录的详情面板；
 * 演示阶段给 REQ-142 挂一条待决审批（Detail Pending 态），
 * 审批动作属业务域，骨架阶段仅占位。
 */
const router = useRouter();
const store = useNotificationsStore();

const selectedId = ref<string | null>(null);

const selected = computed(
  () => store.records.find(record => record.id === selectedId.value) ?? null
);

/** 演示审批态：仅 REQ-142 待决（接真实审批流后由业务数据驱动） */
const approval = computed(() => {
  if (selected.value?.id === "ntf-req-142") {
    return { state: "pending" as const };
  }
  return undefined;
});

function onOpen(record: NotificationRecord) {
  router.push(record.target);
}

function onReview() {
  // 审批流属业务域（S02 I1 人工确认链路），骨架阶段不接线
}
</script>

<template>
  <PageFrame>
    <div class="notifications-view">
      <NotificationCenter
        v-model:selected-id="selectedId"
        class="notifications-view__center"
        @open="onOpen"
      />
      <NotificationDetail
        v-if="selected"
        class="notifications-view__detail"
        :record="selected"
        :approval="approval"
        @open="onOpen"
        @review="onReview"
        @mark-read="store.markRead(selected.id)"
      />
    </div>
  </PageFrame>
</template>

<style scoped>
.notifications-view {
  display: flex;
  align-items: flex-start;
  gap: var(--space-4);
}

.notifications-view__center {
  flex: 1;
  min-width: 0;
}

.notifications-view__detail {
  flex: none;
  width: var(--size-notification-filter-width);
}
</style>
