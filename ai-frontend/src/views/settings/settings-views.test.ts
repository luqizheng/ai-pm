// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import type { Component } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { DOMWrapper } from "@vue/test-utils";

import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import SettingsAiView from "./SettingsAiView.vue";
import SettingsLandingView from "./SettingsLandingView.vue";
import SettingsNotificationsView from "./SettingsNotificationsView.vue";
import SettingsPreferencesView from "./SettingsPreferencesView.vue";
import SettingsProfileView from "./SettingsProfileView.vue";
import SettingsSecurityView from "./SettingsSecurityView.vue";

/**
 * 应用设置视图契约（Settings Framework 个人域，/settings/* 应用壳内）：
 *   - /settings 重定向到个人域落地页；
 *   - 路由无 meta.layout（WorkspaceShell 应用壳内渲染，非设计目录）；
 *   - 落地页五类目卡 + 侧栏条目指向应用路径 /settings/*；
 *   - 每节可挂载且渲染关键内容（≥1 条行为用例/节，与设计目录共用节组件）。
 *
 * 注：确认弹窗 Teleport 到 body，断言与点击走 document.body。
 */
const mountedWrappers: VueWrapper[] = [];

async function mountAt(view: Component, path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(view, { global: { plugins: [router] } });
  mountedWrappers.push(wrapper);
  return { wrapper, router };
}

function bodyButtons(): HTMLButtonElement[] {
  return [...document.body.querySelectorAll("button")];
}

function bodyText(): string {
  return document.body.textContent ?? "";
}

afterEach(() => {
  // 先卸载组件（含 Teleport/reka portal），再清理 body 残留
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

describe("应用设置路由", () => {
  it("/settings 重定向到个人域落地页", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push("/settings");
    await router.isReady();
    expect(router.currentRoute.value.path).toBe("/settings/personal");
  });

  it("设置路由无 meta.layout（应用壳内渲染，非设计目录）", async () => {
    const { router } = await mountAt(
      SettingsProfileView,
      "/settings/personal/profile"
    );
    expect(router.currentRoute.value.meta.layout).toBeUndefined();
  });
});

describe("落地页", () => {
  it("五类目卡，且卡片与侧栏条目指向应用路径 /settings/*", async () => {
    const { wrapper } = await mountAt(
      SettingsLandingView,
      "/settings/personal"
    );
    const cards = wrapper.findAll(".personal-landing__card");
    expect(cards).toHaveLength(5);
    expect(cards.map(card => card.attributes("href"))).toEqual([
      "/settings/personal/profile",
      "/settings/personal/security",
      "/settings/personal/preferences",
      "/settings/personal/notifications",
      "/settings/personal/ai"
    ]);

    // 侧栏导航项同样指向应用路径（设计目录路径不得泄漏进应用壳）
    const links = wrapper
      .findAll("a.settings-nav-item")
      .map(a => a.attributes("href"));
    expect(links).toHaveLength(5);
    expect(links.every(href => href?.startsWith("/settings/personal/"))).toBe(
      true
    );
  });
});

describe("个人域五节（共享节组件）", () => {
  it("资料页：编辑后保存条浮现，取消回滚", async () => {
    const { wrapper } = await mountAt(
      SettingsProfileView,
      "/settings/personal/profile"
    );
    expect(wrapper.find(".settings-savebar").exists()).toBe(false);

    const input = wrapper.find("#profile-display-name");
    await input.setValue("韩林");
    expect(wrapper.find(".settings-savebar").exists()).toBe(true);

    const cancel = wrapper
      .findAll("button")
      .find(b => b.text().includes("取消"));
    await cancel!.trigger("click");
    expect(wrapper.find(".settings-savebar").exists()).toBe(false);
    expect(
      (wrapper.find("#profile-display-name").element as HTMLInputElement).value
    ).toBe("Han Lin");
  });

  it("安全页：吊销会话走 danger 确认后移除该行", async () => {
    const { wrapper } = await mountAt(
      SettingsSecurityView,
      "/settings/personal/security"
    );
    const rowsBefore = wrapper.findAll("tbody tr").length;
    expect(rowsBefore).toBe(3);

    const revoke = wrapper
      .findAll("button")
      .find(b => b.text().includes("吊销"));
    await revoke!.trigger("click");
    expect(bodyText()).toContain("吊销此会话？");

    const confirmButtons = bodyButtons().filter(b =>
      b.textContent?.includes("吊销")
    );
    await new DOMWrapper(confirmButtons.at(-1)!).trigger("click");
    expect(wrapper.findAll("tbody tr").length).toBe(rowsBefore - 1);
  });

  it("偏好页：主题 radio 真实切换应用主题（html data-theme + aipm.theme 持久化）", async () => {
    // 与 index.html 防闪烁脚本同键；先固定深色起点避免用例间污染
    localStorage.setItem("aipm.theme", "dark");
    document.documentElement.dataset.theme = "dark";

    const { wrapper } = await mountAt(
      SettingsPreferencesView,
      "/settings/personal/preferences"
    );
    // 初始选中态反映当前持久化主题（此前硬编码 dark）
    const darkInput = wrapper.find('input[type=radio][value="dark"]');
    expect(darkInput.element.checked).toBe(true);

    const lightInput = wrapper.find('input[type=radio][value="light"]');
    expect(lightInput.exists()).toBe(true);
    await lightInput.setValue();
    // 应用页（apply-to-app）：写 html data-theme + 持久化，与 UserMenu 外观同链路
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem("aipm.theme")).toBe("light");
    // 应用模式下不再落本地演示作用域
    expect(wrapper.find(".preferences-theme[data-theme=light]").exists()).toBe(
      false
    );

    await wrapper.find('input[type=radio][value="dark"]').setValue();
    expect(document.documentElement.dataset.theme).toBe("dark");

    // 快捷键表只读呈现
    expect(wrapper.find(".preferences-shortcuts").exists()).toBe(true);

    localStorage.removeItem("aipm.theme");
    document.documentElement.dataset.theme = "dark";
  });

  it("通知页：邮件通道禁用（Proposed），类别开关可切换", async () => {
    const { wrapper } = await mountAt(
      SettingsNotificationsView,
      "/settings/personal/notifications"
    );
    const switches = wrapper.findAll("[role=switch]");
    expect(switches.length).toBeGreaterThanOrEqual(6);
    const disabled = switches.filter(
      s => s.attributes("disabled") !== undefined
    );
    expect(disabled.length).toBe(2);

    const firstInactive = switches.find(
      s =>
        s.attributes("disabled") === undefined &&
        s.attributes("aria-checked") === "false"
    );
    if (firstInactive) {
      await firstInactive.trigger("click");
      expect(firstInactive.attributes("aria-checked")).toBe("true");
    }
  });

  it("AI 偏好页：三种交互方式 + 边界说明", async () => {
    const { wrapper } = await mountAt(SettingsAiView, "/settings/personal/ai");
    expect(wrapper.findAll(".ui-radio")).toHaveLength(3);
    expect(wrapper.text()).toContain("个人 AI 偏好只影响你本人");
  });
});
