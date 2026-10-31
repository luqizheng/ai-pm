// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const pagePermissions = vi.fn();

vi.mock("@/shell/api/iam", () => ({
  iam: {
    permissions: {
      page: (...args: unknown[]) => pagePermissions(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("权限管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    pagePermissions.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    pagePermissions.mockResolvedValueOnce({
      items: [
        {
          id: "1",
          code: "IAM_USER_READ",
          name: "读取用户",
          description: "d",
          category: "1",
          resource: "user",
          action: "read",
          sort_order: 1,
          status: "active",
          created_at: "2026-01-01",
          updated_at: "2026-01-01"
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(pagePermissions).toHaveBeenCalled();
    expect(w.text()).toContain("IAM_USER_READ");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
