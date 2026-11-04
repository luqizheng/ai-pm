// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import NotificationDetail from "./NotificationDetail.vue";
import type { NotificationRecord } from "./types";
import { i18n } from "@/lib/i18n";

/**
 * 通知详情（Global/Notification/Detail）：审批指示器三态 +
 * 操作行条件渲染（处理请求仅待决 / 标记已读仅未读）。
 */
describe("NotificationDetail", () => {
  const record: NotificationRecord = {
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

  function mountDetail(props: Record<string, unknown>) {
    return mount(NotificationDetail, {
      props,
      global: { plugins: [i18n] }
    });
  }

  it("渲染标题 / 分类 / 元数据 / 主题与上下文", () => {
    const wrapper = mountDetail({ record });

    expect(wrapper.find(".notification-detail__category").text()).toBe(
      "人工审批请求"
    );
    expect(wrapper.find(".notification-detail__meta").text()).toContain(
      "未读"
    );
    expect(wrapper.find(".notification-detail__subject").text()).toBe(
      "Decision requested on REQ-142"
    );
  });

  it("待决审批：指示器 + 处理请求（primary）+ 标记已读", async () => {
    const wrapper = mountDetail({
      record,
      approval: { state: "pending" }
    });

    expect(wrapper.find(".approval-indicator").classes()).toContain(
      "is-pending"
    );

    const review = wrapper.findAll("button").find(b => b.text() === "处理请求")!;
    await review.trigger("click");
    expect(wrapper.emitted("review")).toBeTruthy();

    const mark = wrapper.findAll("button").find(b => b.text() === "标记已读")!;
    await mark.trigger("click");
    expect(wrapper.emitted("mark-read")).toBeTruthy();
  });

  it("已批准：无「处理请求」按钮，指示器声明非 AI 批准", () => {
    const wrapper = mountDetail({
      record,
      approval: { state: "approved", actor: "Maya" }
    });

    expect(
      wrapper.findAll("button").some(b => b.text() === "处理请求")
    ).toBe(false);
    expect(wrapper.find(".approval-indicator__desc").text()).toContain(
      "非 AI 批准"
    );
  });

  it("已读记录：不渲染「标记已读」", () => {
    const wrapper = mountDetail({
      record: { ...record, read: true }
    });

    expect(
      wrapper.findAll("button").some(b => b.text() === "标记已读")
    ).toBe(false);
  });

  it("打开对象发出 open 事件", async () => {
    const wrapper = mountDetail({ record });

    const open = wrapper
      .findAll("button")
      .find(b => b.text() === "打开 REQ-142")!;
    await open.trigger("click");

    expect(wrapper.emitted("open")?.[0]?.[0]).toMatchObject({ id: record.id });
  });
});
