// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import NotificationRecord from "./NotificationRecord.vue";
import type { NotificationRecord as RecordType } from "./types";
import { i18n } from "@/lib/i18n";

/**
 * 通知记录块（Global/Notification/Record）：分类行 + Human attention 徽标 +
 * 对象操作行；compact（下拉内）隐藏操作行；selected = accent 面 + accent 描边。
 */
describe("NotificationRecord", () => {
  const record: RecordType = {
    id: "ntf-task-318",
    category: "taskAssignment",
    requiresAttention: true,
    title: "TASK-318 assigned to you",
    description: "Add token expiry coverage for REQ-142.",
    timestamp: "8m ago",
    object: "Atlas Platform · TASK-318",
    openLabel: "TASK-318",
    target: "/project/execution",
    read: false
  };

  function mountRecord(props: Record<string, unknown> = {}) {
    return mount(NotificationRecord, {
      props: { record, ...props },
      global: { plugins: [i18n] }
    });
  }

  it("渲染分类名 + Human attention 徽标（需人工处理显式标注）", () => {
    const wrapper = mountRecord();

    expect(wrapper.find(".notification-record__category").text()).toBe(
      "人工任务指派"
    );
    expect(wrapper.find(".ui-badge").text()).toBe("需人工处理");
  });

  it("无需人工处理的记录不渲染徽标", () => {
    const wrapper = mountRecord({
      record: { ...record, requiresAttention: false }
    });

    expect(wrapper.find(".ui-badge").exists()).toBe(false);
  });

  it("操作行：打开对象 / 标记已读（未读时）", async () => {
    const wrapper = mountRecord();

    const buttons = wrapper.findAll("button");
    const open = buttons.find(b => b.text() === "打开 TASK-318")!;
    const mark = buttons.find(b => b.text() === "标记已读")!;

    await open.trigger("click");
    expect(wrapper.emitted("open")?.[0]?.[0]).toMatchObject({ id: record.id });

    await mark.trigger("click");
    expect(wrapper.emitted("toggle-read")).toBeTruthy();
  });

  it("已读记录的操作为「标记未读」", () => {
    const wrapper = mountRecord({ record: { ...record, read: true } });

    expect(
      wrapper.findAll("button").some(b => b.text() === "标记未读")
    ).toBe(true);
  });

  it("selected：accent 着色面 + accent 描边", () => {
    const wrapper = mountRecord({ selected: true });

    expect(wrapper.find(".notification-record").classes()).toContain(
      "is-selected"
    );
  });
});
