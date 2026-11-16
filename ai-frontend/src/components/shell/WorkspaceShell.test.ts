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
    // 生效态 data-*：列被移除时网格同步收列（否则主区右侧留 AI 列宽空带）
    expect(wrapper.attributes("data-ai-panel")).toBe("closed");
    expect(wrapper.attributes("data-context")).toBe("closed");
    // 专注模式对管理页无意义（本无 AI 面板可收）：顶栏不出现专注开关
    expect(wrapper.find(".shell-topbar__focus").exists()).toBe(false);
  });

  it.each(["/project/settings", "/project/settings/repositories"])(
    "项目设置页（%s）：AI 助手相关 chrome 都不出现",
    async path => {
      const router = createRouter({
        history: createMemoryHistory(),
        routes: buildRoutes()
      });
      await router.push(path);
      await router.isReady();
      const { wrapper } = mountShell(WorkspaceShell, {}, router);
      await nextTick();

      // 与 /admin/settings 同口径：顶栏入口、AI 列、上下文栏、专注开关齐收
      expect(wrapper.find(".workspace-chat").exists()).toBe(false);
      expect(wrapper.find(".workspace-context").exists()).toBe(false);
      expect(wrapper.find(".shell-topbar__assistant").exists()).toBe(false);
      expect(wrapper.find(".shell-topbar__focus").exists()).toBe(false);
      expect(wrapper.find(".workspace-main").exists()).toBe(true);
      // 生效态 data-*：网格同步收列，避免右侧留 AI 列宽空带
      expect(wrapper.attributes("data-ai-panel")).toBe("closed");
      expect(wrapper.attributes("data-context")).toBe("closed");
    }
  );

  it.each(["/settings/team/members", "/settings/team/ai-providers"])(
    "team settings admin page (%s): assistant chrome hidden (C4)",
    async path => {
      const router = createRouter({
        history: createMemoryHistory(),
        routes: buildRoutes()
      });
      await router.push(path);
      await router.isReady();
      const { wrapper } = mountShell(WorkspaceShell, {}, router);
      await nextTick();

      expect(wrapper.find(".workspace-chat").exists()).toBe(false);
      expect(wrapper.find(".shell-topbar__assistant").exists()).toBe(false);
      expect(wrapper.attributes("data-ai-panel")).toBe("closed");
      expect(wrapper.attributes("data-context")).toBe("closed");
    }
  );

  it.each([
    "/settings/team/general",
    "/settings/team",
    "/settings/team/credentials"
  ])(
    "team settings non-admin page (%s): assistant chrome visible (4a lock)",
    async path => {
      const router = createRouter({
        history: createMemoryHistory(),
        routes: buildRoutes()
      });
      await router.push(path);
      await router.isReady();
      const { wrapper } = mountShell(WorkspaceShell, {}, router);
      await nextTick();

      expect(wrapper.find(".workspace-chat").exists()).toBe(true);
      expect(wrapper.find(".shell-topbar__assistant").exists()).toBe(true);
      expect(wrapper.attributes("data-ai-panel")).toBe("open");
    }
  );

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

  it("AI 面板：关闭按钮只收上下文栏（原位可恢复）；顶栏「助手」补开 / 全关 / 全开", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    // AI 列头部的「关闭上下文栏」：仅收右栏，助手列保留（三列布局）
    await wrapper.find(".workspace-chat__context-toggle").trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-context")).toBe("closed");
    expect(wrapper.find(".workspace-chat").exists()).toBe(true);
    expect(wrapper.find(".workspace-context").exists()).toBe(false);

    // 原位恢复按钮：再点 → 上下文栏恢复
    await wrapper.find(".workspace-chat__context-toggle").trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-context")).toBe("open");
    expect(wrapper.find(".workspace-context").exists()).toBe(true);

    // 全开时顶栏「助手」→ 整体关闭（两列齐收，主区独占）
    await wrapper.find(".shell-topbar__assistant").trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-ai-panel")).toBe("closed");
    expect(wrapper.find(".workspace-chat").exists()).toBe(false);
    expect(wrapper.find(".workspace-context").exists()).toBe(false);

    // 再点 → 全开
    await wrapper.find(".shell-topbar__assistant").trigger("click");
    await nextTick();

    expect(wrapper.attributes("data-ai-panel")).toBe("open");
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
