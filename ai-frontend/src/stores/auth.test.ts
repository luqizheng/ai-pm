// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { authGateway, getMe } = vi.hoisted(() => ({
  authGateway: {
    signIn: vi.fn(),
    signOut: vi.fn(async () => {}),
    requestPasswordReset: vi.fn(async () => {}),
    resetPassword: vi.fn(async () => {}),
    acceptInvitation: vi.fn()
  },
  getMe: vi.fn()
}));

vi.mock("@/api/auth", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/auth")>();
  return { ...actual, authGateway };
});

vi.mock("@/api/base/self-service", () => ({ getMe }));

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
    expect(auth.user).toBeNull();
  });

  /** /me 响应（昵称已在别处改过；/me 不含 roles/permissions） */
  const PROFILE = {
    id: "u-1",
    username: "alex.morgan@acme.dev",
    email: "alex@acme.dev",
    phone: null,
    nickname: "Alex M.",
    avatar: "/v1/file/f-1",
    status: "active",
    email_verified: true,
    phone_verified: false,
    created_at: "2026-08-11T00:00:00Z",
    updated_at: "2026-11-07T00:00:00Z"
  };

  async function signedInStore() {
    setActivePinia(createPinia());
    const auth = useAuthStore();
    authGateway.signIn.mockResolvedValueOnce(SESSION);
    await auth.signIn({
      account: "alex.morgan@acme.dev",
      password: "Orbit-Research!26",
      remember: false
    });
    return auth;
  }

  it("登录后 user 为登录响应的资料快照（账户菜单数据源）", async () => {
    const auth = await signedInStore();

    expect(auth.user?.nickname).toBe("Alex Morgan");
    expect(auth.user?.email).toBe("alex.morgan@acme.dev");
  });

  it("refreshProfile 以 /me 校正资料、保留登录响应的 roles 并持久化", async () => {
    const auth = await signedInStore();
    getMe.mockResolvedValueOnce(PROFILE);

    await auth.refreshProfile();

    expect(auth.user).toMatchObject({
      nickname: "Alex M.",
      email: "alex@acme.dev",
      avatar: "/v1/file/f-1",
      roles: ["member"]
    });
    const persisted = JSON.parse(
      window.localStorage.getItem("prism_user") ?? "null"
    );
    expect(persisted?.nickname).toBe("Alex M.");
    expect(persisted?.roles).toEqual(["member"]);
  });

  it("本会话已刷新则不重复拉取 /me（force 例外）", async () => {
    const auth = await signedInStore();
    getMe.mockResolvedValue(PROFILE);

    await auth.refreshProfile();
    await auth.refreshProfile();
    expect(getMe).toHaveBeenCalledTimes(1);

    await auth.refreshProfile(true);
    expect(getMe).toHaveBeenCalledTimes(2);
  });

  it("/me 不可达时保留登录快照且不抛错", async () => {
    const auth = await signedInStore();
    getMe.mockRejectedValueOnce(new Error("offline"));

    await expect(auth.refreshProfile()).resolves.toBeUndefined();
    expect(auth.user?.nickname).toBe("Alex Morgan");
  });

  it("applyProfile 直接同步资料编辑结果（不发请求）", async () => {
    const auth = await signedInStore();

    auth.applyProfile({ ...PROFILE, nickname: "Alex Renamed" });

    expect(auth.user?.nickname).toBe("Alex Renamed");
    expect(getMe).not.toHaveBeenCalled();
  });
});
