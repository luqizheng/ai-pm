// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { authGateway } = vi.hoisted(() => ({
  authGateway: {
    signIn: vi.fn(),
    signOut: vi.fn(async () => {}),
    requestPasswordReset: vi.fn(async () => {}),
    resetPassword: vi.fn(async () => {}),
    acceptInvitation: vi.fn()
  }
}));

vi.mock("@/api/auth", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/auth")>();
  return { ...actual, authGateway };
});

import { AuthError } from "@/api/auth";
import { useAuthStore } from "./auth";

/**
 * 认证 store：会话写入、失败码落库、记住我的账号持久化。
 * 网关用 fake 注入（真实网关属集成测试范畴）。
 */
const SESSION = {
  account: "alex.morgan@acme.dev",
  organization: "Acme Research",
  user: {
    id: "u-1",
    username: "alex.morgan@acme.dev",
    nickname: "Alex Morgan",
    email: "alex.morgan@acme.dev",
    phone: null,
    avatar: null,
    roles: ["member"],
    permissions: []
  }
};

describe("auth store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  it("登录成功后写入会话", async () => {
    setActivePinia(createPinia());
    const auth = useAuthStore();
    authGateway.signIn.mockResolvedValueOnce(SESSION);

    await auth.signIn({
      account: "alex.morgan@acme.dev",
      password: "Orbit-Research!26",
      remember: false
    });

    expect(auth.signedIn).toBe(true);
    expect(auth.account).toBe("alex.morgan@acme.dev");
    expect(auth.organization).toBe("Acme Research");
  });

  it("凭据错误时落库失败码并保持登出态", async () => {
    setActivePinia(createPinia());
    const auth = useAuthStore();
    authGateway.signIn.mockRejectedValueOnce(
      new AuthError("invalid_credentials")
    );

    await expect(
      auth.signIn({
        account: "locked@acme.dev",
        password: "Orbit-Research!26",
        remember: false
      })
    ).rejects.toThrow();

    expect(auth.signedIn).toBe(false);
    expect(auth.errorCode).toBe("invalid_credentials");
  });

  it("记住我时持久化账号", async () => {
    setActivePinia(createPinia());
    const auth = useAuthStore();
    authGateway.signIn.mockResolvedValueOnce(SESSION);

    await auth.signIn({
      account: "alex.morgan@acme.dev",
      password: "Orbit-Research!26",
      remember: true
    });

    expect(auth.rememberedAccount).toBe("alex.morgan@acme.dev");
    expect(window.localStorage.getItem("aipm.auth.remembered-account")).toBe(
      "alex.morgan@acme.dev"
    );
  });

  it("登出清空会话", async () => {
    setActivePinia(createPinia());
    const auth = useAuthStore();
    authGateway.signIn.mockResolvedValueOnce(SESSION);

    await auth.signIn({
      account: "alex.morgan@acme.dev",
      password: "Orbit-Research!26",
      remember: false
    });
    await auth.signOut();

    expect(auth.signedIn).toBe(false);
    expect(auth.account).toBeNull();
  });
});
