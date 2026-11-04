// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";

import WorkspaceShell from "./WorkspaceShell.vue";
import { mountShell } from "./test-utils";

/**
 * 壳根行为：五区域布局、折叠开关的命令面板/指令栏快捷键。
 */
describe("WorkspaceShell", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("渲染五个区域：顶栏 / 左导航 / 主区 / 右上下文栏 / 指令栏", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    expect(wrapper.find(".workspace-topbar").exists()).toBe(true);
    expect(wrapper.find(".workspace-leftnav").exists()).toBe(true);
    expect(wrapper.find(".workspace-main").exists()).toBe(true);
    expect(wrapper.find(".workspace-context").exists()).toBe(true);
    expect(wrapper.find(".workspace-commandbar").exists()).toBe(true);
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
