// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";

import SearchForm from "./SearchForm.vue";

/**
 * 统一搜索过滤行：三槽契约（#search 限宽 / 默认筛选横排 / #append 右对齐）。
 * 竖排缺陷的回归锚点：Select 触发器 width:100% 根样式必须被容器覆盖，
 * 筛选控件与搜索框同处一行（样式契约对组件源码断言，同 layout.test 套路）。
 */
describe("SearchForm", () => {
  const Filter = defineComponent({
    name: "Filter",
    setup: () => () => h("button", { class: "filter-stub" }, "filter")
  });

  it("渲染搜索槽、默认筛选与右对齐追加槽", () => {
    const wrapper = mount(SearchForm, {
      slots: {
        search: { template: "<input class='search-stub' />" },
        default: () => h(Filter),
        append: { template: "<span class='append-stub'>view</span>" }
      }
    });

    // 搜索槽：有内容才渲染，且包在限宽容器里
    const search = wrapper.find(".search-form__search");
    expect(search.exists()).toBe(true);
    expect(search.find(".search-stub").exists()).toBe(true);

    // 默认插槽子项直接落在根容器（横排 flex 子项，不包额外层级）
    expect(wrapper.find(".filter-stub").exists()).toBe(true);
    expect(wrapper.find(".search-form > .filter-stub").exists()).toBe(true);

    // 右对齐槽
    expect(wrapper.find(".search-form__append .append-stub").exists()).toBe(
      true
    );
  });

  it("空槽不渲染对应容器", () => {
    const wrapper = mount(SearchForm, {
      slots: { default: () => h(Filter) }
    });
    expect(wrapper.find(".search-form__search").exists()).toBe(false);
    expect(wrapper.find(".search-form__append").exists()).toBe(false);
  });

  it("样式契约：筛选子项按内容取宽（覆盖 Select 触发器 width:100% 的竖排根因）", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/ui/search-form/SearchForm.vue"),
      "utf8"
    ).replace(/\/\*[\s\S]*?\*\//g, "");

    // 竖排根因回归锚点：width:100% 的触发器子项必须被改为内容宽
    expect(source).toMatch(
      /\.search-form > :not\(\.search-form__search\):not\(\.search-form__append\)\s*\{/
    );
    expect(source).toMatch(/width: max-content/);
    // 搜索槽限宽与右对齐槽
    expect(source).toMatch(
      /\.search-form__search\s*\{[^}]*width: var\(--size-list-search-width\)/
    );
    expect(source).toMatch(/\.search-form__append\s*\{[^}]*margin-left: auto/);
  });
});
