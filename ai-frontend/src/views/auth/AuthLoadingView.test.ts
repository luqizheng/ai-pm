// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

import AuthLoadingView from "./AuthLoadingView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { waitForRoute } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { AUTH_PATHS } from "@/router/auth";

/**
 * 认证进行中页行为：步骤按节奏推进、当前步带 aria-current、可取消返回登录。
 */
describe("AuthLoadingView", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("当前步骤带 aria-current 并随节奏推进", async () => {
    vi.useFakeTimers();
    const { wrapper, router } = mountShell(AuthLoadingView);
    await router.isReady();
    await nextTick();

    expect(wrapper.find('[aria-current="step"]').text()).toContain(
      i18n.global.t("auth.loading.stepSession")
    );

    vi.advanceTimersByTime(1200);
    await nextTick();

    expect(wrapper.find('[aria-current="step"]').text()).toContain(
      i18n.global.t("auth.loading.stepWorkspace")
    );
  });

  it("完成后进入工作区", async () => {
    vi.useFakeTimers();
    const { router } = mountShell(AuthLoadingView);
    await router.isReady();
    await nextTick();

    vi.advanceTimersByTime(3600);
    await flushPromises();

    // /personal 是懒加载路由（导航树登记了业务页组件），首次导航需经
    // Vite 真实 I/O 加载 —— 假定时器下无法等待，必须切回真实定时器再轮询。
    vi.useRealTimers();
    await waitForRoute(router, "/personal");
    expect(router.currentRoute.value.path).toBe("/personal");
  });

  it("可取消并返回登录页", async () => {
    const { wrapper, router } = mountShell(AuthLoadingView);
    await router.isReady();
    await nextTick();

    const cancel = wrapper
      .findAll("button")
      .find(button =>
        button.text().includes(i18n.global.t("auth.loading.cancel"))
      );
    expect(cancel).toBeTruthy();

    await cancel!.trigger("click");
    await waitForRoute(router, AUTH_PATHS.login);

    expect(router.currentRoute.value.path).toBe(AUTH_PATHS.login);
  });
});
