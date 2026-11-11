// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

import UserMenuAccountMenu from "./UserMenuAccountMenu.vue";
import { i18n } from "@/lib/i18n";

/**
 * 账户菜单（AccountMenu 448 面板）：身份块 + 六项操作 + 退出登录。
 * （原「外观」子面板已上移顶栏 ThemeToggle。）
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

  it("未加入团队如实显示；无邮箱 / 无项目角色时不渲染对应行", async () => {
    const wrapper = mount(UserMenuAccountMenu, {
      attachTo: document.body,
      props: { name: "admin" },
      global: { plugins: [i18n] }
    });
    await nextTick();
    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();

    expect(panel()?.querySelector(".user-menu__name")?.textContent).toBe(
      "admin"
    );
    expect(panel()?.querySelector(".user-menu__email")).toBeNull();
    const context = Array.from(
      panel()?.querySelectorAll(".user-menu__context") ?? []
    ).map(el => el.textContent?.trim());
    expect(context).toEqual(["未加入团队"]);
  });

  it("七项操作齐备，退出登录为末项", async () => {
    const wrapper = mountMenu();
    await nextTick();
    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();

    const labels = panelButtons().map(b => b.textContent?.trim());
    expect(labels).toEqual([
      "我的资料",
      "账户设置",
      "团队设置",
      "安全",
      "个人偏好",
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

  it("键盘快捷键 / 退出登录走专属事件", async () => {
    const wrapper = mountMenu();
    await nextTick();
    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();

    panelButtons()
      .find(b => b.textContent?.trim() === "键盘快捷键")!
      .click();
    await nextTick();
    expect(wrapper.emitted("shortcuts")).toBeTruthy();

    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();
    panelButtons()
      .find(b => b.textContent?.trim() === "退出登录")!
      .click();
    await nextTick();
    expect(wrapper.emitted("sign-out")).toBeTruthy();
  });
});
