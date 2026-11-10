// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import DependencyGraphView from "./DependencyGraphView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

/**
 * 执行规划 · 依赖图（S04-UI-05，mock 预览页）交互模型测试：
 * 分层图渲染、选择高亮 + 检查器、依赖增删四类校验（自/未知/重复/环）、
 * 依赖校验问题面板、并行组视图与 AI 并行化采纳。
 */

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/planning/graph");
  await router.isReady();
  const wrapper = mountShell(
    DependencyGraphView,
    { attachTo: document.body },
    router
  ).wrapper;
  await flushPromises();
  return { wrapper, router };
}

describe("DependencyGraphView", () => {
  it("渲染分层依赖图（节点/关键路径标记/图例）", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="pg-canvas"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="pg-node-T-001"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="pg-node-T-011"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="pg-node-T-001"]').classes()).toContain(
      "is-critical"
    );
    expect(wrapper.find('[data-testid="pg-node-T-003"]').classes()).toContain(
      "is-parallel"
    );
  });

  it("选择节点联动检查器：上游依赖与依赖原因", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="pg-node-T-011"]').trigger("click");
    await flushPromises();

    const inspector = wrapper.find('[data-testid="pg-inspector"]');
    expect(inspector.exists()).toBe(true);
    expect(inspector.text()).toContain("T-007");
    expect(inspector.text()).toContain("T-009");
    expect(wrapper.find('[data-testid="pg-dep-remove-T-007"]').exists()).toBe(
      true
    );
  });

  it("添加依赖四类校验：自依赖 / 未知编号 / 重复 / 环路", async () => {
    const { wrapper } = await mountView();

    // 自依赖
    await wrapper.find('[data-testid="pg-node-T-002"]').trigger("click");
    await wrapper.find('[data-testid="pg-dep-input"] input').setValue("T-002");
    await wrapper.find('[data-testid="pg-dep-add"]').trigger("click");
    expect(wrapper.find('[data-testid="pg-dep-error"]').text()).toContain(
      "自身"
    );

    // 未知编号
    await wrapper.find('[data-testid="pg-dep-input"] input').setValue("T-099");
    await wrapper.find('[data-testid="pg-dep-add"]').trigger("click");
    expect(wrapper.find('[data-testid="pg-dep-error"]').text()).toContain(
      "不存在"
    );

    // 重复
    await wrapper.find('[data-testid="pg-dep-input"] input').setValue("T-001");
    await wrapper.find('[data-testid="pg-dep-add"]').trigger("click");
    expect(wrapper.find('[data-testid="pg-dep-error"]').text()).toContain(
      "已存在"
    );

    // 环路：T-001 ← … ← T-009，再让 T-001 依赖 T-009 成环
    await wrapper.find('[data-testid="pg-node-T-001"]').trigger("click");
    await wrapper.find('[data-testid="pg-dep-input"] input').setValue("T-009");
    await wrapper.find('[data-testid="pg-dep-add"]').trigger("click");
    expect(wrapper.find('[data-testid="pg-dep-error"]').text()).toContain(
      "循环"
    );

    // 合法依赖生效：T-003 → T-001（无环）
    await wrapper.find('[data-testid="pg-node-T-003"]').trigger("click");
    await wrapper.find('[data-testid="pg-dep-input"] input').setValue("T-001");
    await wrapper.find('[data-testid="pg-dep-add"]').trigger("click");
    expect(wrapper.find('[data-testid="pg-dep-error"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="pg-dep-remove-T-001"]').exists()).toBe(
      true
    );
  });

  it("依赖校验发现未知引用与同模块冲突", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="pg-validate"]').trigger("click");
    await flushPromises();

    const panel = wrapper.find('[data-testid="pg-validation"]');
    expect(panel.exists()).toBe(true);
    expect(panel.text()).toContain("T-060");
    expect(panel.text()).toContain("T-014");
    expect(panel.text()).toContain("T-015");
  });

  it("并行组视图渲染分组；采纳 AI 并行化建议落地新组", async () => {
    const { wrapper } = await mountView();

    await wrapper
      .find('[data-testid="pg-rec-R-1"] [type="button"]')
      .trigger("click");
    await flushPromises();

    await wrapper.find('[data-testid="pg-mode-groups"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-testid="pg-groups"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="pg-group-G-A"]').text()).toContain(
      "T-001"
    );
    expect(wrapper.text()).toContain("前后端并行组");
  });
});
