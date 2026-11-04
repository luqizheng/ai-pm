// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import SessionExpiredView from "./SessionExpiredView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { waitForRoute } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { AUTH_PATHS } from "@/router/auth";
import { useAuthStore } from "@/stores/auth";

/**
 * 会话过期页行为：回显上次账号与组织、重新登录回到登录页。
 */
describe("SessionExpiredView", () => {
  it("回显上次登录的账号与组织", async () => {
    const { wrapper, router } = mountShell(SessionExpiredView);
    await router.isReady();
    const auth = useAuthStore();
    auth.account = "alex.morgan@acme.dev";
    auth.organization = "Acme Research";
    await nextTick();

    expect(wrapper.text()).toContain("alex.morgan@acme.dev");
    expect(wrapper.text()).toContain("Acme Research");
  });

  it("说明已保存的工作仍在", async () => {
    const { wrapper, router } = mountShell(SessionExpiredView);
    await router.isReady();
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.sessionExpired.infoTitle")
    );
  });

  it("重新登录回到登录页并结束会话", async () => {
    const { wrapper, router } = mountShell(SessionExpiredView);
    await router.isReady();
    const auth = useAuthStore();
    auth.account = "alex.morgan@acme.dev";
    await nextTick();

    const button = wrapper
      .findAll("button")
      .find(item =>
        item.text().includes(i18n.global.t("auth.sessionExpired.signInAgain"))
      );
    expect(button).toBeTruthy();

    await button!.trigger("click");
    await waitForRoute(router, AUTH_PATHS.login);

    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.login);
    expect(auth.signedIn).toBe(false);
  });
});
