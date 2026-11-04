// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import Radio from "./Radio.vue";

describe("Radio", () => {
  it("label 渲染为文本", () => {
    const w = mount(Radio, { props: { label: "Only assigned to me" } });
    expect(w.text()).toContain("Only assigned to me");
  });

  it("modelValue 命中 value 时选中", () => {
    const w = mount(Radio, {
      props: { label: "A", value: "a", modelValue: "a" }
    });
    expect(w.find("input[type='radio']").element.checked).toBe(true);
  });

  it("点击未选项时发出 update:modelValue", async () => {
    const w = mount(Radio, {
      props: { label: "B", value: "b", modelValue: "a", name: "scope" }
    });
    await w.find("input[type='radio']").setValue();
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["b"]);
  });

  it("disabled 时不发事件且 input 禁用", async () => {
    const w = mount(Radio, {
      props: { label: "C", value: "c", modelValue: "a", disabled: true }
    });
    expect(w.find("input[type='radio']").attributes("disabled")).toBeDefined();
  });
});
