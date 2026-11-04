// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { Moon, Sun } from "lucide-vue-next";

import ThemeToggle from "./ThemeToggle.vue";
import { i18n } from "@/lib/i18n";
import { THEME_STORAGE_KEY, applyTheme } from "@/lib/theme";

/**
 * 主题快捷开关（顶栏铃铛旁）：
 * 单击深/浅往返；状态经 lib/theme 共享响应式，与设置偏好页同源。
 */
describe("ThemeToggle", () => {
  function mountToggle() {
    return mount(ThemeToggle, { global: { plugins: [i18n] } });
  }

  beforeEach(() => {
    document.body.innerHTML = "";
    localStorage.removeItem(THEME_STORAGE_KEY);
    document.documentElement.removeAttribute("data-theme");
    // 共享主题状态是模块级 ref（跨用例存活），归零到深色默认
    applyTheme("dark");
  });

  it("深色下显示 Sun，aria-label 指向浅色；单击写 data-theme 并持久化", async () => {
    const wrapper = mountToggle();

    expect(wrapper.findComponent(Sun).exists()).toBe(true);
    expect(wrapper.find(".theme-toggle").attributes("aria-label")).toBe(
      "切换到浅色模式"
    );

    await wrapper.find(".theme-toggle").trigger("click");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
  });

  it("浅色下显示 Moon，单击转回深色（往返）", async () => {
    applyTheme("light");
    const wrapper = mountToggle();

    expect(wrapper.findComponent(Moon).exists()).toBe(true);
    expect(wrapper.find(".theme-toggle").attributes("aria-label")).toBe(
      "切换到深色模式"
    );

    await wrapper.find(".theme-toggle").trigger("click");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
  });

  it("与共享主题状态同源：外部 applyTheme（如设置偏好页）后图标与文案跟随", async () => {
    const wrapper = mountToggle();
    expect(wrapper.findComponent(Sun).exists()).toBe(true);

    applyTheme("light");
    await nextTick();
    expect(wrapper.findComponent(Moon).exists()).toBe(true);
    expect(wrapper.find(".theme-toggle").attributes("aria-label")).toBe(
      "切换到深色模式"
    );
  });
});
