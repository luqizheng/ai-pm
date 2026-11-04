import { computed, ref } from "vue";
import { defineStore } from "pinia";

import {
  demoNotificationsLoader,
  type NotificationsLoader
} from "@/components/global/notification/demo";
import type { NotificationRecord } from "@/components/global/notification/types";

/**
 * 通知中心状态（Global/Notification 族共享）。
 *
 * 顶栏铃铛（未读计数）、通知下拉、通知中心三处消费同一份记录；
 * 标记已读/未读、全部已读在此落地，加载走注入的 loader
 * （骨架阶段 = 演示数据，接真实接口时换实现）。
 */
export const useNotificationsStore = defineStore("notifications", () => {
  const records = ref<NotificationRecord[]>([]);
  const status = ref<"idle" | "loading" | "ready" | "error">("idle");

  const unreadCount = computed(
    () => records.value.filter(record => !record.read).length
  );

  /** 加载事件流；loader rejected → error（下拉/中心的 Retry 重新调用） */
  async function load(loader: NotificationsLoader = demoNotificationsLoader) {
    status.value = "loading";
    try {
      records.value = await loader();
      status.value = "ready";
    } catch {
      status.value = "error";
    }
  }

  function markRead(id: string) {
    const record = records.value.find(item => item.id === id);
    if (record) record.read = true;
  }

  function markUnread(id: string) {
    const record = records.value.find(item => item.id === id);
    if (record) record.read = false;
  }

  function markAllRead() {
    for (const record of records.value) record.read = true;
  }

  return {
    records,
    status,
    unreadCount,
    load,
    markRead,
    markUnread,
    markAllRead
  };
});
