// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import CommandInput from "./CommandInput.vue";

describe("CommandInput", () => {
  it("输入双向绑定", async () => {
    const w = mount(CommandInput, { props: { modelValue: "" } });
    await w.find("input").setValue("summarize findings");
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["summarize findings"]);
  });

  it("Enter 发送，Shift+Enter 不发送", async () => {
    const w = mount(CommandInput, { props: { modelValue: "x" } });
    await w.find("input").trigger("keydown", { key: "Enter" });
    expect(w.emitted("send")).toHaveLength(1);
    await w.find("input").trigger("keydown", { key: "Enter", shiftKey: true });
    expect(w.emitted("send")).toHaveLength(1);
  });

  it("sendDisabled 时发送按钮禁用", () => {
    const w = mount(CommandInput, {
      props: { modelValue: "x", sendDisabled: true }
    });
    expect(
      w.find(".ui-command-input__send").attributes("disabled")
    ).toBeDefined();
  });

  it("loading 时 spinner 替换 sparkles 且 aria-busy", () => {
    const w = mount(CommandInput, { props: { loading: true } });
    expect(w.attributes("aria-label")).toBeUndefined();
    expect(w.find("input").attributes("aria-busy")).toBe("true");
  });

  it("context 插槽渲染", () => {
    const w = mount(CommandInput, {
      props: {},
      slots: {
        context: {
          template: "<span data-test='ctx'>Atlas · Development</span>"
        }
      }
    });
    expect(w.find("[data-test='ctx']").text()).toBe("Atlas · Development");
  });
});
