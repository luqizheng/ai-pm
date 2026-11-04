// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import PageFrame from "./PageFrame.vue";
import { mountShell } from "./test-utils";

/**
 * 统一页框行为：标题区按需渲染、插槽透传、内容落位。
 */
describe("PageFrame", () => {
  it("渲染标题 / 描述与默认插槽内容", () => {
    const { wrapper } = mountShell(PageFrame, {
      props: { title: "页面标题", description: "页面描述" },
      slots: { default: "<p>页面内容</p>" }
    });

    expect(wrapper.find(".shell-page-header__title").text()).toBe("页面标题");
    expect(wrapper.find(".shell-page-header__description").text()).toBe(
      "页面描述"
    );
    expect(wrapper.find(".shell-page-frame").text()).toContain("页面内容");
  });

  it("不传 title 时不渲染标题区，只渲染内容（工作台布局用）", () => {
    const { wrapper } = mountShell(PageFrame, {
      slots: { default: "<p>双栏工作台</p>" }
    });

    expect(wrapper.find(".shell-page-header").exists()).toBe(false);
    expect(wrapper.find(".shell-page-frame").text()).toContain("双栏工作台");
  });

  it("#actions 插槽透传到标题区", () => {
    const { wrapper } = mountShell(PageFrame, {
      props: { title: "带操作" },
      slots: { actions: "<button>新建</button>" }
    });

    expect(wrapper.find(".shell-page-header__actions button").text()).toBe(
      "新建"
    );
  });

  it("面包屑透传给标题区", () => {
    const { wrapper } = mountShell(PageFrame, {
      props: {
        title: "创建项目",
        breadcrumbs: [{ label: "项目", to: "/projects" }, { label: "创建" }]
      }
    });

    expect(wrapper.find(".shell-page-header__breadcrumb").exists()).toBe(true);
  });
});
