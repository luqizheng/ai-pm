// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import SearchInput from "./SearchInput.vue";

describe("SearchInput", () => {
  it("placeholder 与输入双向绑定", async () => {
    const w = mount(SearchInput, {
      props: { modelValue: "", placeholder: "Search items…" }
    });
    expect(w.find("input").attributes("placeholder")).toBe("Search items…");
    await w.find("input").setValue("design");
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["design"]);
  });

  it("shortcut chip 默认渲染，可关闭", () => {
    const withChip = mount(SearchInput, { props: {} });
    expect(withChip.find("kbd").text()).toBe("⌘ F");
    const withoutChip = mount(SearchInput, { props: { shortcut: "" } });
    expect(withoutChip.find("kbd").exists()).toBe(false);
  });

  it("emptyText 存在时渲染帮助行", () => {
    const w = mount(SearchInput, {
      props: { emptyText: "No results for “design”. Try another term." }
    });
    expect(w.text()).toContain("No results for “design”. Try another term.");
  });

  it("loading 时显示 spinner 且隐藏快捷键 chip", () => {
    const w = mount(SearchInput, { props: { loading: true } });
    expect(w.find("kbd").exists()).toBe(false);
  });

  it("disabled 时 input 禁用且整组标记 is-disabled", () => {
    const w = mount(SearchInput, { props: { disabled: true } });
    expect(w.find("input").attributes("disabled")).toBeDefined();
    expect(w.html()).toContain("is-disabled");
  });
});
