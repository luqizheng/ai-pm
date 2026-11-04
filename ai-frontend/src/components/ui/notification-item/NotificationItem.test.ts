// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import NotificationItem from "./NotificationItem.vue";

describe("NotificationItem", () => {
  it("渲染标题与正文", () => {
    const w = mount(NotificationItem, {
      props: {
        title: "Review requested",
        body: "Alex requested your review on PR #128."
      }
    });
    expect(w.text()).toContain("Review requested");
    expect(w.text()).toContain("Alex requested your review on PR #128.");
  });

  it("unread 时渲染 accent 状态点", () => {
    const w = mount(NotificationItem, { props: { title: "N", unread: true } });
    expect(w.find(".ui-status__dot").exists()).toBe(true);
    expect(w.html()).toContain("is-unread");
  });

  it("非未读且有 icon 时渲染图标而非状态点", () => {
    const w = mount(NotificationItem, {
      props: { title: "N", unread: false }
    });
    expect(w.find(".ui-status__dot").exists()).toBe(false);
  });

  it("timestamp 渲染为 time 元素", () => {
    const w = mount(NotificationItem, {
      props: { title: "N", timestamp: "2h ago" }
    });
    expect(w.find("time").text()).toBe("2h ago");
  });

  it("点击发出 click", async () => {
    const w = mount(NotificationItem, { props: { title: "N" } });
    await w.trigger("click");
    expect(w.emitted("click")).toHaveLength(1);
  });
});
