// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import PageHeader from "./PageHeader.vue";
import { mountShell } from "./test-utils";

/**
 * 页面标题区行为（Application Shell §2C）。
 */
describe("PageHeader", () => {
  it("渲染标题与描述", async () => {
    const { wrapper } = mountShell(PageHeader, {
      props: { title: "代码仓库", description: "浏览共享代码与文档" }
    });
    await nextTick();

    expect(wrapper.find(".shell-page-header__title").text()).toBe("代码仓库");
    expect(wrapper.find(".shell-page-header__description").text()).toBe(
      "浏览共享代码与文档"
    );
  });

  it("无描述时不渲染描述段", async () => {
    const { wrapper } = mountShell(PageHeader, {
      props: { title: "代码仓库" }
    });
    await nextTick();

    expect(wrapper.find(".shell-page-header__description").exists()).toBe(
      false
    );
  });

  it("有面包屑时渲染面包屑条目", async () => {
    const { wrapper } = mountShell(PageHeader, {
      props: {
        title: "代码仓库",
        breadcrumbs: [{ label: "项目" }, { label: "Atlas Platform" }]
      }
    });
    await nextTick();

    expect(wrapper.findAll(".ui-breadcrumb__item").length).toBe(2);
  });

  it("渲染页面级操作插槽", async () => {
    const { wrapper } = mountShell(PageHeader, {
      props: { title: "代码仓库" },
      slots: { actions: "<button class='probe-action'>打开</button>" }
    });
    await nextTick();

    expect(wrapper.find(".probe-action").exists()).toBe(true);
  });
});
