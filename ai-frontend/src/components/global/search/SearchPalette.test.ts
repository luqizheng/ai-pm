// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

import SearchPalette from "./SearchPalette.vue";
import { demoSearchIndex, demoSearchFn } from "./demo";
import type { RecentSearch, SearchCommand } from "./types";
import { i18n } from "@/lib/i18n";

/**
 * 全局搜索面板：状态机（idle / searching / results / no-results / error）、
 * 键盘导航（↑↓ 焦点独立于选中、Enter 打开、Esc 关闭）、最近搜索回填。
 */
describe("SearchPalette", () => {
  const commands: SearchCommand[] = [
    {
      id: "cmd-open-project",
      label: "Open project · Atlas Platform",
      group: "navigate",
      icon: "folder",
      target: "/project/command-center"
    },
    {
      id: "cmd-navigate-repositories",
      label: "Navigate module · Repositories",
      group: "navigate",
      icon: "gitBranch",
      target: "/project/settings/repositories"
    },
    {
      id: "cmd-create-project",
      label: "Create project",
      group: "create",
      icon: "plus",
      target: "/projects"
    },
    {
      id: "cmd-open-personal-workspace",
      label: "Open Personal Workspace",
      group: "action",
      icon: "user",
      target: "/personal"
    },
    {
      id: "cmd-open-inbox",
      label: "Open Inbox",
      group: "action",
      icon: "inbox",
      target: "/inbox"
    }
  ];

  const recents: RecentSearch[] = [
    { id: "recent-1", label: "Atlas Platform" },
    { id: "recent-2", label: "REQ-142 · Session timeout" },
    { id: "recent-3", label: "Repository · atlas-api" }
  ];

  function mountPalette(overrides: Record<string, unknown> = {}) {
    return mount(SearchPalette, {
      props: {
        open: true,
        commands,
        recentSearches: recents,
        searchFn: demoSearchFn,
        ...overrides
      },
      global: { plugins: [i18n] }
    });
  }

  /** 走完防抖 + 演示搜索的 250ms 延时 */
  async function settle() {
    await vi.advanceTimersByTimeAsync(160);
    await vi.advanceTimersByTimeAsync(300);
    await nextTick();
  }

  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("idle：渲染命令分组（全局命令/创建）、附加命令面板、最近搜索与键盘提示条", () => {
    const wrapper = mountPalette();

    const labels = wrapper
      .findAll(".global-search__group-label")
      .map(el => el.text());
    expect(labels).toContain("全局命令");
    expect(labels).toContain("创建");

    const options = wrapper.findAll(".global-search__option");
    // navigate(2) + create(1) 主面板 + action(2) 附加面板
    expect(options.length).toBe(5);

    const recentRows = wrapper.findAll(".global-search-recent__row");
    expect(recentRows.length).toBe(3);

    expect(wrapper.find(".global-search__hints").exists()).toBe(true);
  });

  it("typing → searching：输入后进入搜索中（进度 + 骨架行），完成进入 results", async () => {
    const wrapper = mountPalette();

    await wrapper.find("input").setValue("atlas");
    await vi.advanceTimersByTimeAsync(160);
    await nextTick();

    expect(wrapper.find(".global-search__progress").exists()).toBe(true);
    expect(wrapper.findAll(".global-search__skeleton-row").length).toBe(2);

    await settle();
    expect(wrapper.find(".global-search__results").exists()).toBe(true);
    expect(wrapper.find(".global-search__count").text()).toContain("6");
  });

  it("results：按六类分组展示，行带元数据；selected 与键盘焦点互相独立", async () => {
    const wrapper = mountPalette({
      selectedId: demoSearchIndex[0].id // Atlas Platform（project）
    });

    await wrapper.find("input").setValue("atlas");
    await settle();

    const categories = wrapper.findAll(".global-search__category");
    expect(categories.length).toBe(6);

    const rows = wrapper.findAll(".global-search-result__row");
    expect(rows.length).toBe(6);
    // 元数据行随行渲染
    expect(wrapper.findAll(".global-search-result__meta").length).toBe(6);

    // 键盘焦点初始在首行（视觉优先于选中态）
    expect(rows[0].classes()).toContain("is-keyboardFocus");

    // 焦点下移一行后：选中行（Atlas Platform）与键盘焦点行各占一行，互不覆盖
    await rows[1].trigger("keydown", { key: "ArrowDown" });
    const stateClasses = wrapper
      .findAll(".global-search-result__row")
      .map(el => el.classes().find(c => c.startsWith("is-")));
    expect(stateClasses[0]).toBe("is-selected");
    expect(stateClasses[1]).toBe("is-keyboardFocus");
  });

  it("键盘：ArrowDown 移动命令焦点，Enter 打开目标并关闭，Esc 直接关闭", async () => {
    const wrapper = mountPalette();

    await wrapper.trigger("keydown", { key: "ArrowDown" });
    const activeOptions = wrapper.findAll(".global-search__option.is-active");
    expect(activeOptions.length).toBe(1);
    expect(activeOptions[0].text()).toContain("Navigate module");

    await wrapper.trigger("keydown", { key: "Enter" });
    const selectEvents = wrapper.emitted<[string]>("select");
    // ArrowDown 后焦点在第二条命令（仓库命令目标已迁至 Project Setting）
    expect(selectEvents?.[0]?.[0]).toBe("/project/settings/repositories");
    expect(wrapper.emitted("update:open")?.[0]?.[0]).toBe(false);

    const wrapper2 = mountPalette();
    await wrapper2.trigger("keydown", { key: "Escape" });
    expect(wrapper2.emitted("update:open")?.[0]?.[0]).toBe(false);
  });

  it("no-results：无匹配展示空态，清除查询回到 idle", async () => {
    const wrapper = mountPalette();

    await wrapper.find("input").setValue("quantum-scheduler");
    await settle();

    expect(wrapper.find(".global-search__empty").exists()).toBe(true);
    expect(wrapper.find(".global-search__empty-title").text()).toBe("无结果");

    const clear = wrapper.findAll("button").find(b => b.text() === "清除查询")!;
    await clear.trigger("click");
    await nextTick();

    expect(wrapper.find(".global-search__empty").exists()).toBe(false);
    expect(wrapper.findAll(".global-search__option").length).toBe(5);
  });

  it("error：搜索函数 rejected 时展示 danger 反馈框与重试，查询保留", async () => {
    const failing = vi.fn((q: string) =>
      q === "atlas" ? Promise.reject(new Error("boom")) : Promise.resolve([])
    );
    const wrapper = mountPalette({ searchFn: failing });

    await wrapper.find("input").setValue("atlas");
    await settle();

    expect(wrapper.find(".ui-alert").exists()).toBe(true);
    expect(wrapper.find(".ui-alert__title").text()).toBe("搜索不可用");

    const retry = wrapper.findAll("button").find(b => b.text() === "重试")!;
    expect(failing).toHaveBeenCalledTimes(1);
    await retry.trigger("click");
    await settle();
    expect(failing).toHaveBeenCalledTimes(2);
  });

  it("点击最近搜索把该查询回填输入框并重新搜索", async () => {
    const wrapper = mountPalette();

    await wrapper.find(".global-search-recent__row").trigger("click");
    await settle();

    expect((wrapper.find("input").element as HTMLInputElement).value).toBe(
      "Atlas Platform"
    );
    expect(wrapper.find(".global-search__results").exists()).toBe(true);
  });
});
