// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const listSessions = vi.fn();

vi.mock("@/shell/api/iam", () => ({
  iam: {
    auth: {
      listSessions: (...args: unknown[]) => listSessions(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("在线会话", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listSessions.mockResolvedValue([]);
  });

  it("加载并渲染后端返回的会话（裸数组）", async () => {
    listSessions.mockResolvedValueOnce([
      {
        id: "s1",
        user_id: "usr-1",
        client_ip: "10.0.0.1",
        user_agent: null,
        created_at: "2026-01-01",
        expires_at: "2026-02-01",
        is_current: true
      }
    ]);
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(listSessions).toHaveBeenCalled();
    expect(w.text()).toContain("10.0.0.1");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
