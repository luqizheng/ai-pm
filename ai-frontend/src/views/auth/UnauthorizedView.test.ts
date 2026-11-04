// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import UnauthorizedView from "./UnauthorizedView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { waitForRoute } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { AUTH_PATHS } from "@/router/auth";

/**
 * 403 页行为：回显身份与目标工作区、标注状态码、切换账号回登录页。
 */
describe("UnauthorizedView", () => {
  it("回显当前身份与目标工作区，并标注 403", async () => {
    const { wrapper, router } = mountShell(UnauthorizedView);
    await router.isReady();
    await nextTick();

    expect(wrapper.text()).toContain(
      i18n.global.t("auth.unauthorized.signedInAs")
    );
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.unauthorized.requestedWorkspace")
    );
    expect(wrapper.text()).toContain("403");
    expect(wrapper.text()).toContain("ACCESS_DENIED");
  });

  it("切换账号回到登录页", async () => {
    const { wrapper, router } = mountShell(UnauthorizedView);
    await router.isReady();
    await nextTick();

    const button = wrapper
      .findAll("button")
      .find(item =>
        item.text().includes(i18n.global.t("auth.unauthorized.switchAccount"))
      );
    expect(button).toBeTruthy();

    await button!.trigger("click");
    await waitForRoute(router, AUTH_PATHS.login);

    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.login);
  });

  it("返回工作区走应用壳内的总览页", async () => {
    const { wrapper, router } = mountShell(UnauthorizedView);
    await router.isReady();
    await nextTick();

    const button = wrapper
      .findAll("button")
      .find(item =>
        item
          .text()
          .includes(i18n.global.t("auth.unauthorized.backToWorkspaces"))
      );
    expect(button).toBeTruthy();

    await button!.trigger("click");
    await waitForRoute(router, "/personal");

    expect(router.currentRoute.value.path).toBe("/personal");
  });
});
