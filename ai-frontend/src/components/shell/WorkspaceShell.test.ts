// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";

import WorkspaceShell from "./WorkspaceShell.vue";
import { mountShell } from "./test-utils";

/**
 * 壳根行为：四栏布局（DR-C8）、折叠开关、专注模式、命令面板/指令栏快捷键。
 */
describe("WorkspaceShell", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("渲染四栏：顶栏 / 左导航 / 主区 / AI Chat 列 / 右上下文栏", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    expect(wrapper.find(".workspace-topbar").exists()).toBe(true);
    expect(wrapper.find(".workspace-leftnav").exists()).toBe(true);
    expect(wrapper.find(".workspace-main").exists()).toBe(true);
    // AI Chat 是常驻独立列（不再是主列底部的停靠指令栏）
    expect(wrapper.find(".workspace-chat").exists()).toBe(true);
    expect(wrapper.find(".workspace-context").exists()).toBe(true);
    // 指令栏降级为 AI Chat 列内的输入组件（Composer）
    expect(wrapper.find(".workspace-commandbar").exists()).toBe(true);
    expect(wrapper.find(".workspace-main .workspace-commandbar").exists()).toBe(
      false
    );
  });

  it("专注模式：data-focus=on 时收起 AI Chat 与右栏，退出恢复", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    expect(wrapper.attributes("data-focus")).toBe("off");

    // 开关在顶栏：专注模式下 AI Chat 列已收起，顶栏是唯一退出入口
    const toggle = wrapper.find(".shell-topbar__focus");
    expect(toggle.exists()).toBe(true);

    await toggle.trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-focus")).toBe("on");
    expect(wrapper.find(".workspace-chat").exists()).toBe(false);
    expect(wrapper.find(".workspace-context").exists()).toBe(false);
    expect(wrapper.find(".workspace-main").exists()).toBe(true);

    await wrapper.find(".shell-topbar__focus").trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-focus")).toBe("off");
    expect(wrapper.find(".workspace-chat").exists()).toBe(true);
    expect(wrapper.find(".workspace-context").exists()).toBe(true);
  });

  it("默认展开：根元素 data-sidebar=expanded 且右栏可见", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    expect(wrapper.attributes("data-sidebar")).toBe("expanded");
    expect(wrapper.attributes("data-context")).toBe("open");
    expect(wrapper.find(".workspace-context").exists()).toBe(true);
  });

  it("折叠左导航后根元素 data-sidebar=collapsed", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    await wrapper.find(".shell-sidenav__control-button").trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-sidebar")).toBe("collapsed");
  });

  it("关闭右上下文栏后该区域从 DOM 移除且 data-context=closed", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    // 按无障碍名称定位关闭按钮（pin 按钮 aria-label 不同）
    const close = wrapper
      .findAll(".shell-context__icon-button")
      .find(el => el.attributes("aria-label") === "关闭上下文栏");
    expect(close).toBeTruthy();

    await close!.trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-context")).toBe("closed");
    expect(wrapper.find(".workspace-context").exists()).toBe(false);
  });

  it("⌘K / Ctrl+K 打开命令面板，⌘J / Ctrl+J 切换指令栏展开态", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    // 指令栏初始为收起
    expect(wrapper.find(".workspace-commandbar").classes()).not.toContain(
      "is-expanded"
    );

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "j", ctrlKey: true })
    );
    await nextTick();
    expect(wrapper.find(".workspace-commandbar").classes()).toContain(
      "is-expanded"
    );

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "j", ctrlKey: true })
    );
    await nextTick();
    expect(wrapper.find(".workspace-commandbar").classes()).not.toContain(
      "is-expanded"
    );

    // 全局搜索面板：⌘K 打开后再按关闭
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "k", metaKey: true })
    );
    await nextTick();
    expect(wrapper.find(".global-search").exists()).toBe(true);

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "k", metaKey: true })
    );
    await nextTick();
    expect(wrapper.find(".global-search").exists()).toBe(false);
  });

  it("点击顶栏搜索入口打开全局搜索面板", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    await wrapper.find(".shell-topbar__search").trigger("click");
    await nextTick();

    expect(wrapper.find(".global-search").exists()).toBe(true);
  });
});
