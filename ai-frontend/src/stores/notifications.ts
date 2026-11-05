import { computed, ref } from "vue";
import { defineStore } from "pinia";

import {
  listNotifications,
  markAllNotificationsRead,
  markNotificationRead
} from "@/api/notifications";
import type { NotificationRecord } from "@/components/global/notification/types";

/** 通知加载器（测试/故事可注入；默认走真实公告接口） */
export type NotificationsLoader = () => Promise<NotificationRecord[]>;

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
  async function load(loader: NotificationsLoader = listNotifications) {
    status.value = "loading";
    try {
      records.value = await loader();
      status.value = "ready";
    } catch {
      status.value = "error";
    }
  }

  /** 标记已读：先本地反馈，再持久化；失败回滚 */
  async function markRead(id: string) {
    const record = records.value.find(item => item.id === id);
    if (!record || record.read) return;
    record.read = true;
    try {
      await markNotificationRead(id);
    } catch {
      record.read = false;
    }
  }

  /** 标记未读：服务端未提供撤销端点，仅本地生效 */
  function markUnread(id: string) {
    const record = records.value.find(item => item.id === id);
    if (record) record.read = false;
  }

  /** 全部已读：持久化后刷新未读计数 */
  async function markAllRead() {
    const previous = records.value.map(record => record.read);
    for (const record of records.value) record.read = true;
    try {
      await markAllNotificationsRead();
    } catch {
      records.value.forEach((record, index) => {
        record.read = previous[index] ?? false;
      });
    }
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
