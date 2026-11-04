// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import SearchInput from "./SearchInput.vue";

/**
 * 搜索输入框（DS/Forms/SearchInput）：v-model、loading spinner、
 * Esc 芯片、helper 文案、disabled 整体不透明度类。
 */
describe("SearchInput", () => {
  it("v-model 双向绑定", async () => {
    const wrapper = mount(SearchInput, {
      props: { modelValue: "" }
    });

    await wrapper.find("input").setValue("atlas");
    expect(wrapper.emitted("update:modelValue")?.at(-1)?.[0]).toBe("atlas");
  });

  it("loading：渲染 spinner 且不渲染 Esc 芯片", () => {
    const wrapper = mount(SearchInput, {
      props: { modelValue: "", loading: true, escapeChip: true }
    });

    expect(wrapper.find(".global-search-field__spinner").exists()).toBe(true);
    expect(wrapper.find(".global-search-field__kbd").exists()).toBe(false);
  });

  it("非 loading 且 escapeChip：渲染 Esc 芯片", () => {
    const wrapper = mount(SearchInput, {
      props: { modelValue: "", escapeChip: true }
    });

    expect(wrapper.find(".global-search-field__kbd").text()).toBe("Esc");
  });

  it("helperText 渲染在输入框下方", () => {
    const wrapper = mount(SearchInput, {
      props: { modelValue: "", helperText: "没有可访问对象匹配此查询。" }
    });

    expect(wrapper.find(".global-search-field__helper").text()).toBe(
      "没有可访问对象匹配此查询。"
    );
  });

  it("disabled：根元素带 is-disabled 类且 input 不可用", () => {
    const wrapper = mount(SearchInput, {
      props: { modelValue: "", disabled: true }
    });

    expect(wrapper.find(".global-search-field").classes()).toContain(
      "is-disabled"
    );
    expect(
      (wrapper.find("input").element as HTMLInputElement).disabled
    ).toBe(true);
  });
});
