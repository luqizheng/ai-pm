// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";

import { ProjectSwitcher, ProjectSwitcherMenu } from "./index";
import type { ProjectSwitcherProject } from "./project-switcher";

/**
 * ProjectSwitcher —— Figma「06 — Project Context Switcher」（33:82961）。
 * 行为：分区列表 / 当前项 Current / 搜索过滤与 NoResults / loading 骨架 /
 * empty / error Retry / canCreate 权限可见性 / Trigger 五态与开合。
 * 样式契约：Figma 数值必须来自 token（对齐 ui.contract.test.ts 口径）。
 */
const projects: ProjectSwitcherProject[] = [
  { id: "atlas", name: "Atlas Platform", group: "favorites" },
  { id: "sdk", name: "SDK Toolkit", group: "recent", meta: "2h ago" },
  {
    id: "portal",
    name: "Developer Portal",
    group: "recent",
    meta: "Yesterday"
  }
];

async function flush() {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 10));
  await nextTick();
}

const wrappers: VueWrapper[] = [];

function mountMenu(props: Record<string, unknown> = {}) {
  const w = mount(ProjectSwitcherMenu, {
    props: { projects, currentId: "atlas", ...props },
    attachTo: document.body
  });
  wrappers.push(w);
  return w;
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("ProjectSwitcherMenu", () => {
  it("渲染标题 / 搜索占位 / FAVORITES + RECENT 分区与全部项目", () => {
    const w = mountMenu();
    expect(w.text()).toContain("Project Switcher");
    expect(w.find("input").attributes("placeholder")).toBe(
      "Search available projects…"
    );
    expect(w.text()).toContain("Favorites");
    expect(w.text()).toContain("Recent");
    expect(w.text()).toContain("Atlas Platform");
    expect(w.text()).toContain("SDK Toolkit");
    expect(w.text()).toContain("Developer Portal");
  });

  it("当前项目行带 Current 徽标，其余行显示 meta", () => {
    const w = mountMenu();
    const rows = w.findAll(".ps-menu__row");
    expect(rows).toHaveLength(3);
    expect(rows[0].classes()).toContain("is-current");
    expect(rows[0].text()).toContain("Current");
    expect(rows[1].text()).toContain("2h ago");
    expect(rows[2].text()).toContain("Yesterday");
  });

  it("点击行发出 select(id)", async () => {
    const w = mountMenu();
    await w.findAll(".ps-menu__row")[1].trigger("click");
    expect(w.emitted("select")).toEqual([["sdk"]]);
  });

  it("query 过滤分区；无命中时渲染 NoResults 并可 Clear query", async () => {
    const w = mountMenu({ query: "sdk" });
    const rows = w.findAll(".ps-menu__row");
    expect(rows).toHaveLength(1);
    expect(rows[0].text()).toContain("SDK Toolkit");

    await w.setProps({ query: "helios" });
    expect(w.text()).toContain("No matching projects");
    expect(w.text()).toContain("“helios”");
    expect(w.text()).toContain("Clear query");

    await w.find(".ps-menu__feedback button").trigger("click");
    expect(w.emitted("clear-query")).toHaveLength(1);
  });

  it("loading 渲染骨架行且不渲染旧列表（绝不静默切换上下文）", () => {
    const w = mountMenu({ status: "loading" });
    expect(w.findAll(".ps-menu__skeleton-row")).toHaveLength(2);
    expect(w.find(".ps-menu__row").exists()).toBe(false);
    expect(w.find(".ps-menu__create").exists()).toBe(false);
  });

  it("empty 渲染空态；canCreate 时 Create 动作发出 create", async () => {
    const w = mountMenu({ status: "empty" });
    expect(w.text()).toContain("No available projects");
    await w.find(".ps-menu__feedback button").trigger("click");
    expect(w.emitted("create")).toHaveLength(1);
  });

  it("error 渲染 danger Alert + Retry（无 Create）", async () => {
    const w = mountMenu({
      status: "error",
      texts: {
        errorDescription:
          "Current project remains Atlas Platform. Retry to load available projects."
      }
    });
    expect(w.text()).toContain("Projects could not load");
    expect(w.text()).toContain("Current project remains Atlas Platform");
    expect(w.find(".ps-menu__create").exists()).toBe(false);
    const buttons = w.findAll("button");
    const retry = buttons.find(b => b.text() === "Retry");
    expect(retry).toBeTruthy();
    await retry!.trigger("click");
    expect(w.emitted("retry")).toHaveLength(1);
  });

  it("canCreate=false：Create 禁用 + 说明替换为无权限文案", async () => {
    const w = mountMenu({ canCreate: false });
    const create = w.find(".ps-menu__create");
    expect(create.attributes("disabled")).toBeDefined();
    expect(w.text()).toContain(
      "Creation unavailable for this identity. No hidden projects are disclosed."
    );
  });
});

describe("ProjectSwitcher（Trigger + Popover）", () => {
  it("锚点渲染标签与图标；disabled / selected 态类正确", () => {
    const w = mount(ProjectSwitcher, {
      props: { projects, label: "Atlas Platform", disabled: true },
      attachTo: document.body
    });
    wrappers.push(w);
    const trigger = w.find(".ps-trigger");
    expect(trigger.text()).toContain("Atlas Platform");
    expect(trigger.attributes("disabled")).toBeDefined();
    expect(trigger.classes()).not.toContain("is-selected");

    const w2 = mount(ProjectSwitcher, {
      props: { projects, label: "Atlas Platform", selected: true },
      attachTo: document.body
    });
    wrappers.push(w2);
    expect(w2.find(".ps-trigger").classes()).toContain("is-selected");
  });

  it("点击锚点展开菜单；点击项目行发出 select 并收起", async () => {
    const w = mount(ProjectSwitcher, {
      props: { projects, label: "Atlas Platform", currentId: "atlas" },
      attachTo: document.body
    });
    wrappers.push(w);

    expect(w.find(".ps-trigger").attributes("aria-expanded")).toBe("false");
    await w.find(".ps-trigger").trigger("click");
    await flush();

    const menuInDom = document.querySelector(".ps-menu");
    expect(menuInDom).not.toBeNull();
    expect(menuInDom!.textContent).toContain("Project Switcher");

    const rows = Array.from(
      document.querySelectorAll<HTMLElement>(".ps-menu__row")
    );
    rows.find(r => r.textContent!.includes("SDK Toolkit"))!.click();
    await flush();

    expect(w.emitted("select")).toEqual([["sdk"]]);
    expect(w.find(".ps-trigger").attributes("aria-expanded")).toBe("false");
  });

  it("点击 Create 发出 create 并收起浮层", async () => {
    const w = mount(ProjectSwitcher, {
      props: { projects, label: "Atlas Platform", currentId: "atlas" },
      attachTo: document.body
    });
    wrappers.push(w);

    await w.find(".ps-trigger").trigger("click");
    await flush();
    expect(document.querySelector(".ps-menu")).not.toBeNull();

    document.querySelector<HTMLElement>(".ps-menu__create")!.click();
    await flush();

    expect(w.emitted("create")).toHaveLength(1);
    expect(w.find(".ps-trigger").attributes("aria-expanded")).toBe("false");
  });
});

describe("样式契约（Figma 33:82961 数值必须来自 token）", () => {
  it("组件层不出现裸色值，尺寸走 token", () => {
    const dir = resolve(
      process.cwd(),
      "src/components/global/project-switcher"
    );
    for (const file of ["ProjectSwitcher.vue", "ProjectSwitcherMenu.vue"]) {
      const s = readFileSync(resolve(dir, file), "utf8");
      expect(s).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    }
    const menu = readFileSync(resolve(dir, "ProjectSwitcherMenu.vue"), "utf8");
    expect(menu).toContain("var(--size-project-menu)");
    expect(menu).toContain("var(--ws-row-height)");
    expect(menu).toContain("var(--surface-base)");
    expect(menu).toContain("var(--accent)");
    const trigger = readFileSync(resolve(dir, "ProjectSwitcher.vue"), "utf8");
    expect(trigger).toContain("var(--ws-sidebar)");
    expect(trigger).toContain("var(--accent-surface)");
    expect(trigger).toContain("var(--opacity-disabled-control)");
    expect(trigger).toContain("var(--size-control-md)");
  });
});
