// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

import InvitationView from "./InvitationView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import { AUTH_PATHS } from "@/router/auth";

/**
 * 邀请接受页行为：回显邀请详情、接受后进入认证进度页。
 */
describe("InvitationView", () => {
  it("回显邀请上下文（受邀邮箱 / 角色 / 过期时间）", async () => {
    const { wrapper, router } = mountShell(InvitationView);
    await router.isReady();
    await nextTick();

    expect(wrapper.text()).toContain("alex.morgan@acme.dev");
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.invitation.roleLabel")
    );
    expect(wrapper.text()).toContain(i18n.global.t("auth.invitation.role"));
    expect(wrapper.text()).toContain(
      i18n.global.t("auth.invitation.expiresLabel")
    );
  });

  it("接受邀请后进入认证进度页", async () => {
    const { wrapper, router } = mountShell(InvitationView);
    await router.isReady();
    await nextTick();

    await wrapper.find("button[type=submit]").trigger("click");

    await new Promise(resolve => setTimeout(resolve, 600));
    await flushPromises();

    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.signingIn);
  });

  it("提供换账号入口，回到登录页", async () => {
    const { wrapper, router } = mountShell(InvitationView);
    await router.isReady();
    await nextTick();

    const link = wrapper
      .findAll("a")
      .find(anchor =>
        anchor
          .text()
          .includes(i18n.global.t("auth.invitation.useAnotherAccount"))
      );
    expect(link).toBeTruthy();
    expect(link!.attributes("href")).toContain(AUTH_PATHS.login);
  });
});
