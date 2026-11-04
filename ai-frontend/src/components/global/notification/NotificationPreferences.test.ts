// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import NotificationPreferences from "./NotificationPreferences.vue";
import type { NotificationCategory } from "./types";
import { i18n } from "@/lib/i18n";

/**
 * 通知偏好（Global/Notification/Preferences）：六类送达开关。
 */
describe("NotificationPreferences", () => {
  const preferences: Record<NotificationCategory, boolean> = {
    approvalRequests: true,
    taskAssignment: true,
    aiUpdates: true,
    projectEvents: true,
    repositoryEvents: false,
    systemAlerts: true
  };

  it("渲染六类开关，受控值生效（仓库事件关）", () => {
    const wrapper = mount(NotificationPreferences, {
      props: { preferences },
      global: { plugins: [i18n] }
    });

    const switches = wrapper.findAll(".ui-switch");
    expect(switches.length).toBe(6);

    const off = switches.find(el => el.text().includes("仓库事件"))!;
    expect(off.find("[data-state='checked']").exists()).toBe(false);
  });

  it("切换开关发出 toggle（类别 + 新值）", async () => {
    const wrapper = mount(NotificationPreferences, {
      props: { preferences },
      global: { plugins: [i18n] }
    });

    const repoSwitch = wrapper
      .findAll(".ui-switch")
      .find(el => el.text().includes("仓库事件"))!;
    await repoSwitch.find("[role='switch']").trigger("click");

    const events = wrapper.emitted<[NotificationCategory, boolean]>("toggle");
    expect(events?.[0]?.[0]).toBe("repositoryEvents");
    expect(events?.[0]?.[1]).toBe(true);
  });
});
