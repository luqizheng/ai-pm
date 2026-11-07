// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectPlaceholderView from "./SettingsProjectPlaceholderView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

/**
 * Project Setting 占位节（General / Members / AI Configuration）：
 * 标题取路由 meta.title，正文明示「规划中」，不冒充既有能力。
 */
async function mountView(path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push(path);
  await router.isReady();
  const mounted = mountShell(SettingsProjectPlaceholderView, {}, router);
  await flushPromises();
  return mounted;
}

describe("SettingsProjectPlaceholderView", () => {
  it("General 节渲染设置壳与规划中占位", async () => {
    const { wrapper } = await mountView("/project/settings/general");
    expect(wrapper.text()).toContain("基本信息");
    expect(wrapper.text()).toContain("该设置节规划中");
    // 侧栏五节条目 + proposed 徽标（General / Members / AI 三节）
    expect(wrapper.text()).toContain("项目成员");
    expect(wrapper.text()).toContain("Runners");
    expect(wrapper.text()).toContain("仓库");
    expect(wrapper.findAll(".settings-nav-item__proposed")).toHaveLength(3);
  });

  it("AI Configuration 节标题随路由 meta 切换", async () => {
    const { wrapper } = await mountView("/project/settings/ai");
    expect(wrapper.text()).toContain("AI 配置");
    expect(wrapper.text()).toContain("该设置节规划中");
  });

  it("项目名未解析时面包屑省略项目层（不出现「项目 > 项目」）", async () => {
    const { wrapper } = await mountView("/project/settings/general");
    const crumbs = wrapper
      .findAll(".ui-breadcrumb__link, .ui-breadcrumb__current")
      .map(node => node.text());
    expect(crumbs.filter(label => label === "项目")).toHaveLength(1);
  });
});
