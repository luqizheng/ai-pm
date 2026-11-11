// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

import LocaleToggle from "./LocaleToggle.vue";
import { i18n, LOCALE_STORAGE_KEY, setLocale } from "@/lib/i18n";

/**
 * 语言快捷开关（顶栏铃铛旁）：
 * 单击中/英往返，按钮显示目标语言短标签；状态经 lib/i18n 共享响应式，
 * 与设置偏好页同源（setLocale 双向互同步 + aipm-locale 持久化）。
 */
describe("LocaleToggle", () => {
  function mountToggle() {
    return mount(LocaleToggle, { global: { plugins: [i18n] } });
  }

  beforeEach(() => {
    document.body.innerHTML = "";
    localStorage.removeItem(LOCALE_STORAGE_KEY);
    // 共享 locale 是模块级状态（跨用例存活），归零到默认中文
    setLocale("zh-CN");
  });

  it("中文界面显示 EN，aria-label 承诺 English；单击切换并持久化", async () => {
    const wrapper = mountToggle();

    expect(wrapper.find(".locale-toggle").text()).toBe("EN");
    expect(wrapper.find(".locale-toggle").attributes("aria-label")).toBe(
      "切换到 English"
    );

    await wrapper.find(".locale-toggle").trigger("click");
    expect(i18n.global.locale.value).toBe("en");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("en");
  });

  it("英文界面显示 中，单击切回中文（往返）", async () => {
    setLocale("en");
    const wrapper = mountToggle();

    expect(wrapper.find(".locale-toggle").text()).toBe("中");
    expect(wrapper.find(".locale-toggle").attributes("aria-label")).toBe(
      "Switch to 中文"
    );

    await wrapper.find(".locale-toggle").trigger("click");
    expect(i18n.global.locale.value).toBe("zh-CN");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("zh-CN");
  });

  it("与共享 locale 同源：外部 setLocale（如设置偏好页）后标签跟随", async () => {
    const wrapper = mountToggle();
    expect(wrapper.find(".locale-toggle").text()).toBe("EN");

    setLocale("en");
    await nextTick();
    expect(wrapper.find(".locale-toggle").text()).toBe("中");
  });
});
