// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const pageItems = vi.fn();

vi.mock("@/shell/api/config", () => ({
  config: {
    items: {
      page: (...args: unknown[]) => pageItems(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("系统配置", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    pageItems.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    pageItems.mockResolvedValueOnce({
      items: [
        {
          id: "1",
          namespace_id: "ns1",
          key: "db.url",
          value: "postgres://x",
          value_type: "string",
          is_secret: false,
          created_at: "2026-01-01",
          updated_at: "2026-01-01",
          description: null,
          value_masked: false
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(pageItems).toHaveBeenCalled();
    expect(w.text()).toContain("db.url");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
