// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";

import { buildRoutes } from "@/router";
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

  it("管理页（导航「管理」组）：AI Chat 列、顶栏助手入口与右上下文栏都不出现", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push("/admin/settings");
    await router.isReady();
    const { wrapper } = mountShell(WorkspaceShell, {}, router);
    await nextTick();

    // AI Chat 列与其一体的上下文栏、顶栏「助手」按钮都收起；主区照常渲染
    expect(wrapper.find(".workspace-chat").exists()).toBe(false);
    expect(wrapper.find(".workspace-context").exists()).toBe(false);
    expect(wrapper.find(".shell-topbar__assistant").exists()).toBe(false);
    expect(wrapper.find(".workspace-main").exists()).toBe(true);
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

  it("AI 面板一体开关：关闭按钮收起两列，顶栏「助手」再开", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    // 关闭按钮在 AI Chat 列头部（自上下文栏头部移入）
    const close = wrapper.find(".workspace-chat__close");
    expect(close.exists()).toBe(true);

    await close.trigger("click");
    await nextTick();

    // 一体收起：AI Chat 列与右上下文栏同时从 DOM 移除，主区独占
    expect(wrapper.attributes("data-context")).toBe("closed");
    expect(wrapper.find(".workspace-chat").exists()).toBe(false);
    expect(wrapper.find(".workspace-context").exists()).toBe(false);

    // 顶栏「助手」= 同一开关：再点整体重开（默认全开）
    await wrapper.find(".shell-topbar__assistant").trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-context")).toBe("open");
    expect(wrapper.find(".workspace-chat").exists()).toBe(true);
    expect(wrapper.find(".workspace-context").exists()).toBe(true);
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
