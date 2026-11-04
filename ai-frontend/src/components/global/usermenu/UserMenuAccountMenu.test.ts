// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

import UserMenuAccountMenu from "./UserMenuAccountMenu.vue";
import { i18n } from "@/lib/i18n";

/**
 * 账户菜单（AccountMenu 448 面板）：身份块 + 八项操作 + 外观子面板。
 * 浮层经 Portal 挂到 body（仓库约定）。
 */
describe("UserMenuAccountMenu", () => {
  function mountMenu() {
    return mount(UserMenuAccountMenu, {
      attachTo: document.body,
      props: {
        name: "Alex Morgan",
        email: "alex.morgan@acme.dev",
        initials: "AM",
        tenant: "Acme Research",
        projectRole: "Atlas Platform · Contributor"
      },
      global: { plugins: [i18n] }
    });
  }

  function panel() {
    return document.querySelector<HTMLElement>(".user-menu");
  }

  function panelButtons() {
    return Array.from(
      panel()?.querySelectorAll<HTMLButtonElement>("button") ?? []
    );
  }

  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("打开：渲染身份块（姓名/邮箱/组织/项目角色）", async () => {
    const wrapper = mountMenu();
    await nextTick();

    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();

    expect(panel()?.textContent).toContain("Alex Morgan");
    expect(panel()?.textContent).toContain("alex.morgan@acme.dev");
    expect(panel()?.textContent).toContain("组织 · Acme Research");
    expect(panel()?.textContent).toContain("Atlas Platform · Contributor");
  });

  it("八项操作齐备，退出登录为末项", async () => {
    const wrapper = mountMenu();
    await nextTick();
    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();

    const labels = panelButtons().map(b => b.textContent?.trim());
    expect(labels).toEqual([
      "我的资料",
      "账户设置",
      "安全",
      "个人偏好",
      "外观",
      "键盘快捷键",
      "帮助与文档",
      "退出登录"
    ]);
    expect(panelButtons().at(-1)?.className).toContain("is-danger");
  });

  it("点击普通项发出 select(id) 并关闭浮层", async () => {
    const wrapper = mountMenu();
    await nextTick();
    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();

    panelButtons()
      .find(b => b.textContent?.trim() === "账户设置")!
      .click();
    await nextTick();

    expect(wrapper.emitted<[string]>("select")?.[0]?.[0]).toBe(
      "account-settings"
    );
    await new Promise(resolve => setTimeout(resolve, 50));
    expect(panel()).toBeNull();
  });

  it("「外观」切换到子面板（返回项 + Dark/Light），返回回主菜单", async () => {
    const wrapper = mountMenu();
    await nextTick();
    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();

    panelButtons().find(b => b.textContent?.trim() === "外观")!.click();
    await nextTick();

    expect(panel()?.textContent).toContain("返回");
    expect(panel()?.textContent).toContain("深色");
    expect(panel()?.textContent).toContain("浅色");

    panelButtons().find(b => b.textContent?.trim() === "返回")!.click();
    await nextTick();
    expect(panel()?.textContent).toContain("我的资料");
  });

  it("键盘快捷键 / 退出登录走专属事件", async () => {
    const wrapper = mountMenu();
    await nextTick();
    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();

    panelButtons().find(b => b.textContent?.trim() === "键盘快捷键")!.click();
    await nextTick();
    expect(wrapper.emitted("shortcuts")).toBeTruthy();

    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();
    panelButtons().find(b => b.textContent?.trim() === "退出登录")!.click();
    await nextTick();
    expect(wrapper.emitted("sign-out")).toBeTruthy();
  });
});
