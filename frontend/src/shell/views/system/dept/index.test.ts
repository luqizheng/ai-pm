// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const listOrgUnits = vi.fn();

vi.mock("@/shell/api/iam", () => ({
  iam: {
    orgUnits: {
      list: (...args: unknown[]) => listOrgUnits(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("组织架构", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listOrgUnits.mockResolvedValue([]);
  });

  it("加载并渲染组织单元树", async () => {
    listOrgUnits.mockResolvedValueOnce([
      {
        id: "1",
        parent_id: null,
        name: "总部",
        code: "HQ",
        level: 1,
        sort_order: 0,
        status: "active",
        created_at: "2026-01-01",
        updated_at: "2026-01-01",
        children: [
          {
            id: "2",
            parent_id: "1",
            name: "研发部",
            code: "RD",
            level: 2,
            sort_order: 0,
            status: "active",
            created_at: "2026-01-01",
            updated_at: "2026-01-01",
            children: []
          }
        ]
      }
    ]);

    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(listOrgUnits).toHaveBeenCalled();
    expect(w.text()).toContain("总部");
    expect(w.text()).toContain("研发部");
  });
});
