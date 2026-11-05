// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { notificationsApi } = vi.hoisted(() => ({
  notificationsApi: {
    listNotifications: vi.fn(),
    markNotificationRead: vi.fn(async () => {}),
    markAllNotificationsRead: vi.fn(async () => {})
  }
}));

vi.mock("@/api/notifications", () => notificationsApi);

import { demoNotificationRecords } from "@/components/global/notification/demo";
import { useNotificationsStore } from "@/stores/notifications";

/** 通知 store：加载状态机 + 已读联动（顶栏/下拉/中心三处同源）。 */
describe("notifications store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    notificationsApi.listNotifications.mockResolvedValue(
      demoNotificationRecords
    );
  });

  it("初始 idle，load 成功 → ready + 记录落库", async () => {
    const store = useNotificationsStore();
    expect(store.status).toBe("idle");
    expect(store.unreadCount).toBe(0);

    await store.load();
    expect(store.status).toBe("ready");
    expect(store.records.length).toBe(6);
    expect(store.unreadCount).toBe(3);
  });

  it("load rejected → error", async () => {
    const store = useNotificationsStore();

    await store.load(async () => {
      throw new Error("boom");
    });
    expect(store.status).toBe("error");
  });

  it("markRead / markUnread 单条切换并联动未读计数", async () => {
    const store = useNotificationsStore();
    await store.load();

    await store.markRead("ntf-req-142");
    expect(store.unreadCount).toBe(2);

    store.markUnread("ntf-req-142");
    expect(store.unreadCount).toBe(3);
  });

  it("markAllRead 清零未读", async () => {
    const store = useNotificationsStore();
    await store.load();

    await store.markAllRead();
    expect(store.unreadCount).toBe(0);
  });
});
