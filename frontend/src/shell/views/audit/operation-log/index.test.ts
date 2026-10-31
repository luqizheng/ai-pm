// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const queryOperation = vi.fn();

vi.mock("@/shell/api/audit", () => ({
  audit: {
    logs: {
      queryOperation: (...args: unknown[]) => queryOperation(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("操作日志", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryOperation.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    queryOperation.mockResolvedValueOnce({
      items: [
        {
          id: 1,
          user_id: "usr-1",
          action: "update",
          resource: "project",
          created_at: "2026-01-01"
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(queryOperation).toHaveBeenCalled();
    expect(w.text()).toContain("update");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
