// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TaskBreakdownView from "./TaskBreakdownView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

/**
 * 任务拆解（S04-UI-02，mock 预览页）交互模型测试：
 * 生成 → 提案默认待评审 → 批量接受 → 全部评审完成；AI 面板随选中联动。
 * 提案不入库是本页的诚实边界（AI 拆解接口未接线）。
 */

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/breakdown");
  await router.isReady();
  const wrapper = mountShell(
    TaskBreakdownView,
    { attachTo: document.body },
    router
  ).wrapper;
  await flushPromises();
  return { wrapper, router };
}

async function generateProposals(
  wrapper: Awaited<ReturnType<typeof mountView>>["wrapper"]
) {
  await wrapper.find('[data-testid="bd-generate"]').trigger("click");
  // 生成是分批 setTimeout 的流式模拟，快进全部批次
  await vi.advanceTimersByTimeAsync(1000);
  await flushPromises();
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("TaskBreakdownView", () => {
  it("基线卡展示已批准状态，初始为空态并要求已批准基线", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="bd-baseline-status"]').text()).toContain(
      "已批准"
    );
    expect(wrapper.find('[data-testid="bd-row-T-001"]').exists()).toBe(false);
  });

  it("生成分批落行，提案默认待评审（pending）", async () => {
    const { wrapper } = await mountView();
    await generateProposals(wrapper);

    expect(wrapper.find('[data-testid="bd-row-T-001"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="bd-row-T-001.3"]').exists()).toBe(true);
    expect(
      wrapper.find('[data-testid="bd-row-T-001"]').attributes("data-state")
    ).toBe("pending");
    expect(wrapper.find('[data-testid="bd-summary"]').text()).toContain("8");
  });

  it("批量接受所选提案后进入全部评审完成态", async () => {
    const { wrapper } = await mountView();
    await generateProposals(wrapper);

    const boxes = wrapper.findAll('input[type="checkbox"]');
    for (const box of boxes) {
      await box.setValue(true);
    }
    await wrapper.find('[data-testid="bd-accept-selected"]').trigger("click");
    await flushPromises();

    expect(
      wrapper.find('[data-testid="bd-row-T-001"]').attributes("data-state")
    ).toBe("accepted");
    expect(wrapper.find('[data-testid="bd-all-reviewed"]').exists()).toBe(true);
  });

  it("点击任务行联动 AI 拆解面板（来源追溯）", async () => {
    const { wrapper } = await mountView();
    await generateProposals(wrapper);

    await wrapper.find('[data-testid="bd-row-T-001"]').trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("REQ-201");
    expect(wrapper.text()).toContain("ADR-007");
  });
});
