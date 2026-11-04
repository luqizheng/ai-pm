// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import UserMenuItem from "./UserMenuItem.vue";

/** 菜单行（DS/MenuItem）：selected 带 check、danger 文字、disabled。 */
describe("UserMenuItem", () => {
  it("default：点击发出 click", async () => {
    const wrapper = mount(UserMenuItem, { props: { label: "我的资料" } });

    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("click")).toBeTruthy();
  });

  it("selected：accent 面类 + 渲染 check 图标", () => {
    const wrapper = mount(UserMenuItem, {
      props: { label: "深色", variant: "selected" }
    });

    expect(wrapper.find("button").classes()).toContain("is-selected");
    expect(wrapper.find(".user-menu-item__check").exists()).toBe(true);
  });

  it("danger：danger 类且不渲染 check", () => {
    const wrapper = mount(UserMenuItem, {
      props: { label: "退出登录", variant: "danger" }
    });

    expect(wrapper.find("button").classes()).toContain("is-danger");
    expect(wrapper.find(".user-menu-item__check").exists()).toBe(false);
  });

  it("disabled：按钮不可用且不发出 click", async () => {
    const wrapper = mount(UserMenuItem, {
      props: { label: "退出登录", disabled: true }
    });

    expect(
      (wrapper.find("button").element as HTMLButtonElement).disabled
    ).toBe(true);
    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("click")).toBeUndefined();
  });
});
