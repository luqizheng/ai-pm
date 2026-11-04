// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import NotificationBell from "./NotificationBell.vue";
import { i18n } from "@/lib/i18n";

/**
 * 通知铃铛（Global/Notification/Bell 六态）：文本未读徽标、Active、Disabled。
 */
describe("NotificationBell", () => {
  function mountBell(props: Record<string, unknown> = {}) {
    return mount(NotificationBell, {
      props,
      global: { plugins: [i18n] }
    });
  }

  it("无未读：只渲染铃铛按钮，无计数徽标", () => {
    const wrapper = mountBell();

    expect(wrapper.find(".notification-bell__button").exists()).toBe(true);
    expect(wrapper.find(".ui-badge").exists()).toBe(false);
  });

  it("有未读：渲染文本计数徽标（不是色点）", () => {
    const wrapper = mountBell({ unreadCount: 3 });

    expect(wrapper.find(".ui-badge").text()).toBe("3");
  });

  it("active（下拉展开中）→ is-active（accent 着色面）", () => {
    const wrapper = mountBell({ active: true });

    expect(wrapper.find(".notification-bell__button").classes()).toContain(
      "is-active"
    );
  });

  it("disabled → 按钮不可用", () => {
    const wrapper = mountBell({ disabled: true });

    expect(
      (wrapper.find(".notification-bell__button").element as HTMLButtonElement)
        .disabled
    ).toBe(true);
  });

  it("点击发出 click 事件", async () => {
    const wrapper = mountBell();

    await wrapper.find(".notification-bell__button").trigger("click");

    expect(wrapper.emitted("click")).toBeTruthy();
  });
});
