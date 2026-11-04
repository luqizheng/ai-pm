// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";

import SearchResultRow from "./SearchResultRow.vue";
import type { SearchResultItem } from "./types";

/** 搜索结果行：六类对象图标 + 三态（default / selected / keyboardFocus）+ 元数据行 */
describe("SearchResultRow", () => {
  const item: SearchResultItem = {
    id: "search-req-142",
    objectType: "requirement",
    label: "Session timeout",
    description: "REQ-142",
    meta: "Atlas Platform · REQ-142 · In review",
    target: "/project/requirements"
  };

  it("default 态：渲染图标 / 标签 / 编号 / 右箭头，aria-selected=false", () => {
    const wrapper = mount(SearchResultRow, { props: { item } });

    const row = wrapper.find(".global-search-result__row");
    expect(row.classes()).toContain("is-default");
    expect(row.attributes("aria-selected")).toBe("false");
    expect(row.text()).toContain("Session timeout");
    expect(row.text()).toContain("REQ-142");
    expect(wrapper.find(".global-search-result__meta").text()).toContain(
      "In review"
    );
  });

  it("selected 态：accent 着色面且 aria-selected=true", () => {
    const wrapper = mount(SearchResultRow, {
      props: { item, state: "selected" }
    });

    expect(wrapper.find(".global-search-result__row").classes()).toContain(
      "is-selected"
    );
    expect(
      wrapper.find(".global-search-result__row").attributes("aria-selected")
    ).toBe("true");
  });

  it("keyboardFocus 态：2px accent 描边类", () => {
    const wrapper = mount(SearchResultRow, {
      props: { item, state: "keyboardFocus" }
    });

    expect(wrapper.find(".global-search-result__row").classes()).toContain(
      "is-keyboardFocus"
    );
  });

  it("点击行发出 select 事件（携带结果对象）", async () => {
    const wrapper = mount(SearchResultRow, { props: { item } });

    await wrapper.find(".global-search-result__row").trigger("click");

    const events = wrapper.emitted<[SearchResultItem]>("select");
    expect(events?.[0]?.[0].id).toBe(item.id);
  });

  it("无 meta 时不渲染元数据行", () => {
    const wrapper = mount(SearchResultRow, {
      props: { item: { ...item, meta: undefined } }
    });

    expect(wrapper.find(".global-search-result__meta").exists()).toBe(false);
  });
});
