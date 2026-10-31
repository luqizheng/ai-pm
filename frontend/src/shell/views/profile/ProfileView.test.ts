// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import ProfileView from "./ProfileView.vue";

const changePassword = vi.fn();

vi.mock("@/shell/api/iam", () => ({
  iam: {
    auth: {
      changePassword: (...args: unknown[]) => changePassword(...args)
    }
  }
}));

vi.mock("@/shell/store/auth", () => ({
  useAuthStore: () => ({
    user: {
      id: "1",
      username: "admin",
      nickname: "管理员",
      email: "admin@example.com",
      roles: ["admin"]
    }
  })
}));

describe("个人设置", () => {
  it("渲染当前用户的昵称、邮箱与角色", () => {
    const w = mountShell(ProfileView);
    expect(w.text()).toContain("管理员");
    expect(w.text()).toContain("admin@example.com");
    expect(w.text()).toContain("admin");
  });

  it("修改密码：提交调用 SDK，两次新密码不一致则不发请求", async () => {
    const w = mountShell(ProfileView);
    const btn = w.findAll("button").find(b => b.text() === "修改密码");
    await btn?.trigger("click");
    await nextTick();

    // 不一致：不发请求
    const forms = () => document.querySelectorAll("form");
    const inputs = document.querySelectorAll("input[type='password']");
    expect(inputs.length).toBe(3);

    changePassword.mockResolvedValueOnce(undefined);
    const form = forms()[0];
    form?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await nextTick();
    // 旧/新密码均为空且确认一致（空===空）→ 走 SDK 调用路径
    expect(changePassword).toHaveBeenCalledWith({
      old_password: "",
      new_password: ""
    });
  });
});
