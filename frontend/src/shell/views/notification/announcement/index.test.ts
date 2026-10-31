// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const listMine = vi.fn();
const createAnnouncement = vi.fn();
const publishAnnouncement = vi.fn();

vi.mock("@/shell/api/notification", () => ({
  notification: {
    announcements: {
      listMine: (...args: unknown[]) => listMine(...args),
      create: (...args: unknown[]) => createAnnouncement(...args),
      publish: (...args: unknown[]) => publishAnnouncement(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("公告管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMine.mockResolvedValue([]);
  });

  it("加载并渲染后端返回的公告（裸数组）", async () => {
    listMine.mockResolvedValueOnce([
      {
        id: 1,
        title: "停机公告",
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
    expect(w.text()).toContain("停机公告");
  });

  it("草稿行出现发布操作（管理员）", async () => {
    listMine.mockResolvedValueOnce([
      {
        id: 2,
        title: "草稿",
        body: "b",
        publish_at: "2026-01-01",
        pinned: 0,
        popover: 0,
        status: "draft",
        created_by: "admin",
        created_at: "2026-01-01",
        expire_at: null
      }
    ]);
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    const publishBtn = w.findAll("button").find(b => b.text() === "发布");
    expect(publishBtn).toBeDefined();
    await publishBtn?.trigger("click");
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(publishAnnouncement).toHaveBeenCalledWith(2);
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
