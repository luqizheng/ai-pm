// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import LoginPage from "./LoginPage.vue";
import { mountShell } from "../../test-utils";

vi.mock("vue-router", () => ({
  useRoute: () => ({ query: {} }),
  useRouter: () => ({ replace: vi.fn().mockResolvedValue(undefined) })
}));

vi.mock("../../api/client", () => ({ onUnauthorized: vi.fn() }));

const signIn = vi.fn();
vi.mock("../../store/auth", () => ({
  useAuthStore: () => ({ signIn, reset: vi.fn() })
}));

function mountPage() {
  return mountShell(LoginPage, { attachTo: document.body });
}

describe("LoginPage", () => {
  beforeEach(() => {
    signIn.mockReset();
    document.body.innerHTML = "";
  });

  it("渲染产品名、登录表单与安全说明", () => {
    const w = mountPage();
    expect(w.text()).toContain("AI-PM 平台");
    expect(w.text()).toContain("PRISM 控制台");
    expect(w.find("input[type='text']").exists()).toBe(true);
    expect(w.find("input[type='password']").exists()).toBe(true);
    expect(w.find("button[type='submit']").exists()).toBe(true);
    expect(w.text()).toContain("会话凭证仅保存在本机浏览器");
  });

  it("空提交不发请求，逐字段提示并把焦点交给第一个空字段", async () => {
    const w = mountPage();
    await w.find("form").trigger("submit");
    await nextTick();

    expect(signIn).not.toHaveBeenCalled();
    // 字段级报错，而不是一条笼统的「请输入用户名与密码」
    expect(w.text()).toContain("请填写用户名");
    expect(w.text()).toContain("请填写密码");
    expect(w.find("#login-username").attributes("aria-invalid")).toBe("true");
    expect(document.activeElement).toBe(w.find("#login-username").element);
  });

  it("只补用户名时焦点落到密码框", async () => {
    const w = mountPage();
    await w.find("#login-username").setValue("admin");
    await w.find("form").trigger("submit");
    await nextTick();

    expect(signIn).not.toHaveBeenCalled();
    expect(w.text()).not.toContain("请填写用户名");
    expect(document.activeElement).toBe(w.find("#login-password").element);
  });

  it("凭证被拒时显示可读提示，不把服务端错误码抛给用户", async () => {
    signIn.mockRejectedValueOnce({ code: "IAM_AUTH_1002" });
    const w = mountPage();
    await w.find("#login-username").setValue("admin");
    await w.find("#login-password").setValue("wrong");
    await w.find("form").trigger("submit");
    await nextTick();
    await nextTick();

    const alert = w.find("[role='alert']");
    expect(alert.exists()).toBe(true);
    expect(alert.text()).toContain("用户名或密码不正确");
    expect(w.text()).not.toContain("IAM_AUTH_1002");
  });

  it("密码可见性开关同步 input 类型与 aria-pressed", async () => {
    const w = mountPage();
    const toggle = w.find(".ui-login__reveal");

    expect(toggle.attributes("aria-pressed")).toBe("false");
    expect(toggle.attributes("aria-label")).toBe("显示密码");

    await toggle.trigger("click");
    expect(w.find("#login-password").attributes("type")).toBe("text");
    expect(toggle.attributes("aria-pressed")).toBe("true");
    expect(toggle.attributes("aria-label")).toBe("隐藏密码");
  });
});
