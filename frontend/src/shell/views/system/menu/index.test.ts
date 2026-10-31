// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import Page from "./index.vue";

const listMenuConfigs = vi.fn();
const createConfig = vi.fn();

vi.mock("@/shell/api", () => ({
  listMenuConfigs: (...args: unknown[]) => listMenuConfigs(...args),
  publishMenuConfig: vi.fn(),
  cloneMenuConfig: vi.fn()
}));

vi.mock("@/shell/api/prue", () => ({
  prue: {
    configs: {
      create: (...args: unknown[]) => createConfig(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("菜单管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMenuConfigs.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载并渲染接口返回的数据", async () => {
    listMenuConfigs.mockResolvedValueOnce({
      items: [
        {
          id: "1",
          name: "系统管理",
          version: 3,
          is_active: true,
          created_at: "2026-01-01"
        }
      ],
      total: 1
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(listMenuConfigs).toHaveBeenCalled();
    expect(w.text()).toContain("系统管理");
  });

  it("新建版本：提交合法 JSON 走 SDK create", async () => {
    createConfig.mockResolvedValueOnce({
      id: "mc-9",
      version: 1,
      name: "v9",
      description: null,
      status: "draft",
      is_active: false,
      created_by: null,
      created_at: "",
      updated_at: "",
      published_at: null,
      routes_etag: "e9"
    });
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    const create = w.findAll("button").find(b => b.text() === "新建版本");
    await create?.trigger("click");
    await nextTick();

    const form = document.querySelector("form");
    form?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(createConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        name: "",
        routes_json: [expect.objectContaining({ path: "/dashboard" })]
      })
    );
  });

  it("无数据时显示空态", async () => {
    const w = mountShell(Page);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-data-table__empty").exists()).toBe(true);
  });
});
