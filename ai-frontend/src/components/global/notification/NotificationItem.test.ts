// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import NotificationItem from "./NotificationItem.vue";
import type { NotificationRecord } from "./types";
import { i18n } from "@/lib/i18n";

/**
 * 通知条目（DS/Data/NotificationItem）：Unread accent 面 + 未读点，
 * Read surface 面；元数据行 =「状态 · 时间 · 对象」。
 */
describe("NotificationItem", () => {
  const base: NotificationRecord = {
    id: "ntf-req-142",
    category: "approvalRequests",
    requiresAttention: true,
    title: "Decision requested on REQ-142",
    description: "Jonah asks you to review the proposed timeout policy.",
    timestamp: "2m ago",
    object: "Atlas Platform · REQ-142",
    openLabel: "REQ-142",
    target: "/project/requirements",
    read: false
  };

  it("未读：accent 着色面 + 未读点 + 元数据行带「未读」", () => {
    const wrapper = mount(NotificationItem, {
      props: { record: base },
      global: { plugins: [i18n] }
    });

    expect(wrapper.find(".notification-item").classes()).toContain("is-unread");
    expect(wrapper.find(".notification-item__dot").exists()).toBe(true);
    expect(wrapper.find(".notification-item__meta").text()).toContain("未读");
    expect(wrapper.find(".notification-item__meta").text()).toContain(
      "Atlas Platform · REQ-142"
    );
  });

  it("已读：surface 底、无未读点、元数据行带「已读」", () => {
    const wrapper = mount(NotificationItem, {
      props: { record: { ...base, read: true } },
      global: { plugins: [i18n] }
    });

    expect(wrapper.find(".notification-item").classes()).toContain("is-read");
    expect(wrapper.find(".notification-item__dot").exists()).toBe(false);
    expect(wrapper.find(".notification-item__meta").text()).toContain("已读");
  });
});
