// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";

import ProjectSwitcher from "./ProjectSwitcher.vue";
import { mountShell } from "./test-utils";

/**
 * 项目切换器（Figma「04 — Global Navigation」Project Context Header 三态 +
 * 「07 — Project Switching」的切换菜单）。
 *
 * 菜单经 Portal 传送到 body，故用 document 查询（与 ui/dropdown-menu 同约定）。
 */
describe("ProjectSwitcher", () => {
  const projects = [
    { id: "p1", name: "Atlas Platform" },
    { id: "p2", name: "Research Sandbox" }
  ];

  function menuItems() {
    return Array.from(
      document.querySelectorAll<HTMLElement>(".shell-project-menu__item")
    );
  }

  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("已选项目时显示项目名", async () => {
    const { wrapper } = mountShell(ProjectSwitcher, {
      props: { projects, currentId: "p1" },
      attachTo: document.body
    });
    await nextTick();

    expect(wrapper.find(".shell-project-switcher__name").text()).toBe(
      "Atlas Platform"
    );
    expect(wrapper.find(".shell-project-switcher").classes()).not.toContain(
      "is-empty"
    );
  });

  it("未选项目时显示「选择项目」并标记 Empty 态", async () => {
    const { wrapper } = mountShell(ProjectSwitcher, {
      props: { projects, currentId: null },
      attachTo: document.body
    });
    await nextTick();

    expect(wrapper.find(".shell-project-switcher__name").text()).toBe(
      "选择项目"
    );
    expect(wrapper.find(".shell-project-switcher").classes()).toContain(
      "is-empty"
    );
  });

  it("切换中显示目标项目名（不显示旧项目）", async () => {
    const { wrapper } = mountShell(ProjectSwitcher, {
      props: { projects, currentId: "p1", switchingToId: "p2" },
      attachTo: document.body
    });
    await nextTick();

    expect(wrapper.find(".shell-project-switcher__name").text()).toBe(
      "Research Sandbox"
    );
  });

  it("打开菜单后列出全部项目，当前项标记为 Active", async () => {
    const { wrapper } = mountShell(ProjectSwitcher, {
      props: { projects, currentId: "p1" },
      attachTo: document.body
    });
    await nextTick();

    await wrapper.find(".shell-project-switcher").trigger("click");
    await nextTick();

    const items = menuItems();
    expect(items.map(el => el.textContent?.trim())).toEqual([
      "Atlas Platform",
      "Research Sandbox"
    ]);
    expect(items[0].className).toContain("is-active");
    expect(items[1].className).not.toContain("is-active");
  });

  it("菜单含搜索行时按关键词过滤项目", async () => {
    const { wrapper } = mountShell(ProjectSwitcher, {
      props: { projects, currentId: "p1", searchable: true },
      attachTo: document.body
    });
    await nextTick();

    await wrapper.find(".shell-project-switcher").trigger("click");
    await nextTick();

    const input = document.querySelector<HTMLInputElement>(
      ".shell-project-menu__search-input"
    );
    expect(input).not.toBeNull();
    input!.value = "sandbox";
    input!.dispatchEvent(new Event("input"));
    await nextTick();

    const items = menuItems();
    expect(items.length).toBe(1);
    expect(items[0].textContent?.trim()).toBe("Research Sandbox");
  });

  it("切换中显示「正在打开 X…」状态行", async () => {
    const { wrapper } = mountShell(ProjectSwitcher, {
      props: { projects, currentId: "p1", switchingToId: "p2" },
      attachTo: document.body
    });
    await nextTick();

    await wrapper.find(".shell-project-switcher").trigger("click");
    await nextTick();

    expect(
      document.querySelector(".shell-project-menu__status")?.textContent?.trim()
    ).toBe("正在打开 Research Sandbox…");
  });

  it("选择项目后发出 select 事件并关闭菜单", async () => {
    const { wrapper } = mountShell(ProjectSwitcher, {
      props: { projects, currentId: "p1" },
      attachTo: document.body
    });
    await nextTick();

    await wrapper.find(".shell-project-switcher").trigger("click");
    await nextTick();

    const items = menuItems();
    expect(items.length).toBe(2);
    items[1].click();
    await nextTick();

    expect(wrapper.emitted("select")).toEqual([["p2"]]);

    // 浮层卸载走 reka-ui 的 Presence（下一帧才真正移除）
    await new Promise(resolve => setTimeout(resolve, 50));
    expect(document.querySelector(".shell-project-menu")).toBeNull();
  });

  it("chip 变体用于顶栏，header 变体用于左导航", async () => {
    const { wrapper } = mountShell(ProjectSwitcher, {
      props: { projects, currentId: "p1", variant: "chip" },
      attachTo: document.body
    });
    await nextTick();

    expect(wrapper.find(".shell-project-switcher").classes()).toContain(
      "is-chip"
    );
  });
});
