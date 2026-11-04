// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import UserMenuAppearance from "./UserMenuAppearance.vue";
import { i18n } from "@/lib/i18n";

/** 外观面板：Dark/Light 选中态 + 切换写 data-theme 与 aipm.theme。 */
describe("UserMenuAppearance", () => {
  function mountAppearance(modelValue: "dark" | "light" = "dark") {
    return mount(UserMenuAppearance, {
      props: { modelValue },
      global: { plugins: [i18n] }
    });
  }

  it("深色选中：Dark 行为 selected（含 check），Light 为 default", () => {
    const wrapper = mountAppearance("dark");

    const items = wrapper.findAll("button");
    expect(items[0].classes()).toContain("is-selected");
    expect(items[1].classes()).not.toContain("is-selected");
    expect(wrapper.find(".user-menu-appearance__note").text()).toContain(
      "深色"
    );
  });

  it("选择浅色：发出 update:modelValue 并应用 data-theme + 持久化", async () => {
    const wrapper = mountAppearance("dark");

    await wrapper.findAll("button")[1].trigger("click");

    expect(wrapper.emitted("update:modelValue")?.[0]?.[0]).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem("aipm.theme")).toBe("light");

    // 还原，避免污染其他用例
    document.documentElement.dataset.theme = "dark";
    localStorage.setItem("aipm.theme", "dark");
  });
});
