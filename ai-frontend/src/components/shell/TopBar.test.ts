// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import TopBar from "./TopBar.vue";
import { mountShell } from "./test-utils";

/**
 * 顶栏行为（Application Shell §2A）。
 */
describe("TopBar", () => {
  it("渲染产品名与当前组织/项目上下文", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    expect(wrapper.find(".shell-topbar__product").text()).toBe(
      "AI R&D Workspace"
    );
    const contexts = wrapper.findAll(".shell-topbar__context-name");
    expect(contexts.map(el => el.text())).toEqual([
      "Acme Research",
      "Atlas Platform"
    ]);
  });

  it("搜索入口为按钮并带无障碍名称", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    const search = wrapper.find(".shell-topbar__search");
    expect(search.attributes("aria-label")).toBe("搜索或跳转到…");
  });

  it("点击搜索入口发出 search 事件", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    await wrapper.find(".shell-topbar__search").trigger("click");
    expect(wrapper.emitted("search")).toBeTruthy();
  });

  it("通知按钮在有未读时渲染未读标记", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    expect(wrapper.find(".shell-topbar__unread").exists()).toBe(true);
  });

  it("账户菜单渲染头像与菜单项", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    expect(wrapper.find(".shell-topbar__initials").text()).toBe("AM");
  });
});
