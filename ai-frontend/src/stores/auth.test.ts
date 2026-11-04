// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";

import { useAuthStore } from "./auth";

/**
 * 认证 store：会话写入、失败码落库、记住我的账号持久化。
 */
describe("auth store", () => {
  it("登录成功后写入会话", async () => {
    setActivePinia(createPinia());
    const auth = useAuthStore();

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

    await auth.signIn({
      account: "alex.morgan@acme.dev",
      password: "Orbit-Research!26",
      remember: false
    });
    auth.signOut();

    expect(auth.signedIn).toBe(false);
    expect(auth.account).toBeNull();
  });
});
