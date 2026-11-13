import {
  HttpCore,
  type Announcement,
  type CreateAnnouncement
} from "notification-services";

import { notification } from "./base/notification";
import { getAccessToken } from "./base/token-storage";
import { sessionFetch } from "./base/session";
import type {
  NotificationCategory,
  NotificationRecord
} from "@/components/global/notification/types";

/**
 * 通知（以 lmms 公告为主）。
 *
 * `read` 与「全部已读」是本轮 notification-service 新增能力，
 * 上游 SDK 1.10.1 的类型尚未包含，这里做交叉类型与 HttpCore 适配；
 * SDK 升级后可直接删除本层。
 */
export type AnnouncementWithRead = Announcement & { read?: boolean };

const http = new HttpCore({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: sessionFetch
});

/** 公告不区分 PM 六类，统一落到系统提醒 */
const ANNOUNCEMENT_CATEGORY: NotificationCategory = "systemAlerts";

export function toNotificationRecord(
  item: AnnouncementWithRead
): NotificationRecord {
  return {
    id: String(item.id),
    category: ANNOUNCEMENT_CATEGORY,
    requiresAttention: Boolean(item.pinned),
    title: item.title,
    description: item.body,
    timestamp: item.publish_at,
    object: "",
    openLabel: "",
    target: "",
    read: Boolean(item.read)
  };
}

export async function listNotifications(): Promise<NotificationRecord[]> {
  const items = (await notification.announcements.listMine({
    active: true
  })) as AnnouncementWithRead[];
  return items.map(toNotificationRecord);
}

export async function markNotificationRead(id: string): Promise<void> {
  await notification.announcements.markRead(Number(id));
}

export async function markAllNotificationsRead(): Promise<void> {
  await http.post<{ ok: boolean; changed: number }>(
    "/v1/notification/announcements/read-all",
    undefined,
    { auth: true }
  );
}

export function createAnnouncement(
  request: CreateAnnouncement
): Promise<{ id: number }> {
  return notification.announcements.create(request);
}

export function publishAnnouncement(id: number) {
  return notification.announcements.publish(id);
}

export function revokeAnnouncement(id: number) {
  return notification.announcements.revoke(id);
}

export function listAnnouncementsForAdmin(): Promise<AnnouncementWithRead[]> {
  return notification.announcements.listMine() as Promise<
    AnnouncementWithRead[]
  >;
}
