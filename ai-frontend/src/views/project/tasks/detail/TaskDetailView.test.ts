// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TaskDetailView from "./TaskDetailView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

/**
 * 任务详情（S04-UI-03，mock 预览页）信息模型测试：
 * 七区块齐全、审批状态本地切换、AI 建议采纳/忽略。页面不回写后端
 * （无 GET 读端点），这是 mock 阶段的诚实边界。
 */

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/11111111-1111-1111-1111-111111111111");
  await router.isReady();
  const wrapper = mountShell(
    TaskDetailView,
    { attachTo: document.body },
    router
  ).wrapper;
  await flushPromises();
  return { wrapper, router };
}

describe("TaskDetailView", () => {
  it("渲染七区块完整信息模型与来源追溯", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="dt-section-objective"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="dt-section-trace"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="dt-section-deps"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="dt-section-spec"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="dt-section-criteria"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="dt-section-delivery"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="dt-section-history"]').exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("REQ-201");
    expect(wrapper.text()).toContain("ADR-007");
  });

  it("批准动作切换审批状态徽标（本地演示态）", async () => {
    const { wrapper } = await mountView();

    expect(
      wrapper
        .find('[data-testid="dt-chips"] [data-approval]')
        .attributes("data-approval")
    ).toBe("none");

    await wrapper.find('[data-testid="dt-approve"]').trigger("click");

    expect(
      wrapper
        .find('[data-testid="dt-chips"] [data-approval]')
        .attributes("data-approval")
    ).toBe("human_confirmed");
  });

  it("AI 建议支持采纳与忽略，采纳后计数可见", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="dt-suggestion-s1"]').exists()).toBe(
      true
    );

    await wrapper
      .find('[data-testid="dt-suggestion-s1"] button')
      .trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-testid="dt-applied-count"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="dt-suggestion-s1"]').exists()).toBe(
      false
    );
  });
});
