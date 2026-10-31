// @vitest-environment happy-dom

import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import TabBar from "./TabBar.vue";
import { mountShell } from "../test-utils";

const tabs = [
  { path: "/dashboard", title: "概览" },
  { path: "/projects", title: "项目" },
  { path: "/tasks", title: "任务" }
];

async function flushUi() {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 20));
  await nextTick();
}

describe("TabBar（多标签页栏）", () => {
  it("点击标签页发出 select", async () => {
    const w = mountShell(TabBar, {
      props: { tabs, activePath: "/dashboard" }
    });
    await w.findAll(".ui-tabbar__tab")[1].trigger("click");
    expect(w.emitted("select")?.[0]).toEqual(["/projects"]);
  });

  it("点击关闭按钮只发出 close，不触发 select", async () => {
    const w = mountShell(TabBar, {
      props: { tabs, activePath: "/dashboard" }
    });
    await w
      .findAll(".ui-tabbar__tab")[1]
      .find(".ui-tabbar__close")
      .trigger("click");
    expect(w.emitted("close")?.[0]).toEqual(["/projects"]);
    expect(w.emitted("select")).toBeUndefined();
  });

  it("内容溢出时显示左右翻页箭头", async () => {
    const w = mountShell(TabBar, {
      props: { tabs, activePath: "/dashboard" }
    });
    const viewport = w.find(".ui-tabbar__viewport").element as HTMLElement;
    Object.defineProperty(viewport, "scrollWidth", {
      value: 600,
      configurable: true
    });
    Object.defineProperty(viewport, "clientWidth", {
      value: 200,
      configurable: true
    });
    window.dispatchEvent(new Event("resize"));
    await nextTick();

    const navs = w.findAll(".ui-tabbar__nav");
    expect(navs).toHaveLength(2);
    expect(navs[0].attributes("style") ?? "").not.toContain("display: none");
    expect(navs[1].attributes("style") ?? "").not.toContain("display: none");
  });

  it("右键标签页弹出菜单，关闭其他按被右键的路径发出事件", async () => {
    const w = mountShell(TabBar, {
      props: { tabs, activePath: "/dashboard" },
      attachTo: document.body
    });
    await w.findAll(".ui-tabbar__tab")[1].trigger("contextmenu", {
      clientX: 12,
      clientY: 12
    });
    await flushUi();

    const items = [
      ...document.body.querySelectorAll<HTMLElement>("[role='menuitem']")
    ];
    const labels = items.map(item => item.textContent?.trim() ?? "");
    expect(labels).toContain("重新加载");
    expect(labels).toContain("关闭其他");
    expect(labels).toContain("关闭全部");

    const closeOthers = items.find(item =>
      item.textContent?.includes("关闭其他")
    );
    expect(closeOthers).toBeTruthy();
    await (closeOthers as HTMLElement).click();
    await flushUi();

    expect(w.emitted("closeOthers")?.[0]).toEqual(["/projects"]);
    w.unmount();
  });
});
