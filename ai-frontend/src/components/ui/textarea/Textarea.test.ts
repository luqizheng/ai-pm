// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Textarea from "./Textarea.vue";

/** 外壳组件：原生 textarea 在 .ui-textarea__native 内（Figma 容器帧结构） */
function nativeOf(w: ReturnType<typeof mount>) {
  return w.find("textarea");
}

describe("Textarea", () => {
  it("双向绑定 modelValue", async () => {
    const w = mount(Textarea, { props: { modelValue: "a" } });
    expect((nativeOf(w).element as HTMLTextAreaElement).value).toBe("a");
    await nativeOf(w).setValue("b");
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["b"]);
  });

  it("默认 rows 为 4", () => {
    expect(nativeOf(mount(Textarea)).attributes("rows")).toBe("4");
  });

  it("invalid 时附加错误态类并渲染内嵌警示图标", () => {
    const w = mount(Textarea, { props: { invalid: true } });
    expect(w.classes()).toContain("is-invalid");
    expect(w.find(".ui-textarea__icon").exists()).toBe(true);
    expect(nativeOf(w).attributes("aria-invalid")).toBe("true");
    expect(mount(Textarea).find(".ui-textarea__icon").exists()).toBe(false);
  });

  it("disabled 透传到原生元素", () => {
    expect(
      nativeOf(mount(Textarea, { props: { disabled: true } })).attributes(
        "disabled"
      )
    ).toBeDefined();
  });

  it("尺寸映射到约定类名", () => {
    expect(mount(Textarea, { props: { size: "sm" } }).classes()).toContain(
      "ui-textarea--sm"
    );
    expect(mount(Textarea, { props: { size: "lg" } }).classes()).toContain(
      "ui-textarea--lg"
    );
    expect(mount(Textarea).classes()).toContain("ui-textarea--md");
  });
});
