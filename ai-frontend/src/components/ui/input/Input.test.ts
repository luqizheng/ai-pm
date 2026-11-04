// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Input from "./Input.vue";

/** 外壳组件：原生 input 在 .ui-input__native 内（Figma 容器帧结构） */
function nativeOf(w: ReturnType<typeof mount>) {
  return w.find("input");
}

describe("Input", () => {
  it("双向绑定 modelValue", async () => {
    const w = mount(Input, { props: { modelValue: "a" } });
    expect((nativeOf(w).element as HTMLInputElement).value).toBe("a");
    await nativeOf(w).setValue("b");
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["b"]);
  });

  it("invalid 时附加错误态类并输出 aria-invalid=true", () => {
    const w = mount(Input, { props: { invalid: true } });
    expect(w.classes()).toContain("is-invalid");
    expect(nativeOf(w).attributes("aria-invalid")).toBe("true");
    expect(mount(Input).classes()).not.toContain("is-invalid");
    expect(nativeOf(mount(Input)).attributes("aria-invalid")).toBeUndefined();
  });

  it("错误态渲染内嵌警示图标（Figma Error 态）", () => {
    expect(
      mount(Input, { props: { invalid: true } })
        .find(".ui-input__icon--danger")
        .exists()
    ).toBe(true);
    expect(mount(Input).find(".ui-input__icon--danger").exists()).toBe(false);
  });

  it("loading 时渲染内嵌 loader 图标（Figma Loading 态）", () => {
    expect(
      mount(Input, { props: { loading: true } })
        .find(".ui-input__icon")
        .exists()
    ).toBe(true);
  });

  it("disabled 透传到原生元素", () => {
    expect(
      nativeOf(mount(Input, { props: { disabled: true } })).attributes(
        "disabled"
      )
    ).toBeDefined();
  });

  it("尺寸映射到约定类名", () => {
    expect(mount(Input, { props: { size: "sm" } }).classes()).toContain(
      "ui-input--sm"
    );
    expect(mount(Input, { props: { size: "lg" } }).classes()).toContain(
      "ui-input--lg"
    );
    expect(mount(Input).classes()).toContain("ui-input--md");
  });

  it("密码类型启用 mono 字族类（Figma PasswordInput）", () => {
    expect(mount(Input, { props: { type: "password" } }).classes()).toContain(
      "is-mono"
    );
    expect(mount(Input, { props: { type: "text" } }).classes()).not.toContain(
      "is-mono"
    );
  });
});
