// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import UserView from "./index.vue";

const pageUsers = vi.fn();
const createUser = vi.fn();

vi.mock("@/shell/api/iam", () => ({
  iam: {
    users: {
      page: (...args: unknown[]) => pageUsers(...args),
      create: (...args: unknown[]) => createUser(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    hasPermission: () => true,
    hasRole: () => true
  })
}));

describe("系统 / 用户管理", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    pageUsers.mockResolvedValue({ items: [], total: 0 });
  });

  it("加载后渲染接口返回的用户", async () => {
    pageUsers.mockResolvedValueOnce({
      items: [
        {
          id: "1",
          username: "admin",
          nickname: "管理员",
          email: "a@b.c",
          status: "active",
          created_at: "2026-01-01"
        }
      ],
      total: 1
    });
    const w = mountShell(UserView);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    expect(pageUsers).toHaveBeenCalledWith(
      expect.objectContaining({ page: 1, size: 10 })
    );
    expect(w.text()).toContain("admin");
    expect(w.text()).toContain("管理员");
  });

  it("状态列用 Badge 呈现", async () => {
    pageUsers.mockResolvedValueOnce({
      items: [
        {
          id: "1",
          username: "u",
          nickname: "n",
          status: "disabled",
          created_at: ""
        }
      ],
      total: 1
    });
    const w = mountShell(UserView);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();
    expect(w.find(".ui-badge").exists()).toBe(true);
  });

  it("新建时提交 SDK create，创建成功后重新加载列表", async () => {
    pageUsers.mockResolvedValue({ items: [], total: 0 });
    createUser.mockResolvedValueOnce({
      id: "2",
      username: "u",
      nickname: "n",
      status: "active",
      created_at: ""
    });
    const w = mountShell(UserView);
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));
    await nextTick();

    const create = w.findAll("button").find(b => b.text() === "新建用户");
    await create?.trigger("click");
    await nextTick();

    const form = document.querySelector("form");
    form?.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true })
    );
    await nextTick();
    await nextTick();

    expect(createUser).toHaveBeenCalledWith(expect.anything());
    expect(pageUsers).toHaveBeenCalledTimes(2); // 首次加载 + 提交后刷新
  });
});
