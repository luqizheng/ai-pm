// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const pageFiles = vi.fn();

vi.mock("@/shell/api/file", () => ({
  file: {
    files: {
      page: (...args: unknown[]) => pageFiles(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("文件管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    pageFiles.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    pageFiles.mockResolvedValueOnce({
      items: [
        {
          id: "f1",
          filename: "报告.pdf",
          content_type: "application/pdf",
          size: 1024,
          storage_path: "/x",
          tenant_id: 1,
          is_temporary: false,
          created_at: "2026-01-01",
          updated_at: "2026-01-01",
          url: "/v1/file/f1"
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(pageFiles).toHaveBeenCalled();
    expect(w.text()).toContain("报告.pdf");
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
