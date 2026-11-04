// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import UserMenuButton from "./UserMenuButton.vue";
import { i18n } from "@/lib/i18n";

/** 账户触发按钮（AvatarButton 五态 + 首字母头像）。 */
describe("UserMenuButton", () => {
  function mountButton(props: Record<string, unknown> = {}) {
    return mount(UserMenuButton, {
      props: { name: "Alex Morgan", initials: "AM", ...props },
      global: { plugins: [i18n] }
    });
  }

  it("渲染首字母头像与无障碍名称", () => {
    const wrapper = mountButton();

    expect(wrapper.find(".user-menu-button__initials").text()).toBe("AM");
    expect(wrapper.find("button").attributes("aria-label")).toBe("账户菜单");
  });

  it("active（菜单展开中）→ is-active", () => {
    const wrapper = mountButton({ active: true });

    expect(wrapper.find("button").classes()).toContain("is-active");
  });

  it("disabled → 按钮不可用", () => {
    const wrapper = mountButton({ disabled: true });

    expect((wrapper.find("button").element as HTMLButtonElement).disabled).toBe(
      true
    );
  });

  it("点击发出 click", async () => {
    const wrapper = mountButton();

    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("click")).toBeTruthy();
  });
});
