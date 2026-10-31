// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const listMine = vi.fn();
const markRead = vi.fn();

vi.mock("@/shell/api/notification", () => ({
  notification: {
    announcements: {
      listMine: (...args: unknown[]) => listMine(...args),
      markRead: (...args: unknown[]) => markRead(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("公告中心", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMine.mockResolvedValue([]);
  });

  it("加载并渲染我可见的公告", async () => {
    listMine.mockResolvedValueOnce([
      {
        id: 1,
        title: "平台维护窗口公告",
        body: "b",
        publish_at: "2026-01-01",
        pinned: 1,
        popover: 0,
        status: "published",
        created_by: "admin",
        created_at: "2026-01-01",
        expire_at: null
      }
    ]);
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(listMine).toHaveBeenCalled();
    expect(w.text()).toContain("平台维护窗口公告");
  });

  it("行内标记已读后重载", async () => {
    listMine.mockResolvedValue([
      {
        id: 2,
        title: "t",
        body: "b",
        publish_at: "2026-01-01",
        pinned: 0,
        popover: 0,
        status: "published",
        created_by: "admin",
        created_at: "2026-01-01",
        expire_at: null
      }
    ]);
    markRead.mockResolvedValueOnce({ ok: true, changed: true });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    const btn = w
      .findAll("button")
      .find(b => b.text() === "标记已读");
    await btn?.trigger("click");
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(markRead).toHaveBeenCalledWith(2);
    expect(listMine).toHaveBeenCalledTimes(2); // 首次加载 + 已读后刷新
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
