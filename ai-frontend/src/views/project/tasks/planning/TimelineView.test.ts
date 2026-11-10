// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TimelineView from "./TimelineView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

/**
 * 执行规划 · 时间线（S04-UI-06，mock 预览页）交互模型测试：
 * 双栏甘特渲染（任务条/里程碑/未排期托盘）、比例尺切换、拖拽移动
 * （下游顺延预览 + 确认）、排期校验、AI 排期建议采纳、版本保存。
 */

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/planning/timeline");
  await router.isReady();
  const wrapper = mountShell(
    TimelineView,
    { attachTo: document.body },
    router
  ).wrapper;
  await flushPromises();
  return { wrapper, router };
}

function bodyButton(label: string): HTMLButtonElement | null {
  const buttons = Array.from(document.body.querySelectorAll("button"));
  return buttons.find(node => node.textContent?.trim() === label) ?? null;
}

beforeEach(() => {
  vi.useFakeTimers();
});

describe("TimelineView", () => {
  it("渲染任务条/里程碑/未排期托盘", async () => {
    const { wrapper } = await mountView();

    const bar = wrapper.find('[data-testid="tl-bar-T-001"]');
    expect(bar.exists()).toBe(true);
    expect(bar.attributes("data-start")).toBe("2026-11-16");
    expect(wrapper.find('[data-testid="tl-tray-T-010"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("认证基础完成");
  });

  it("比例尺切换改变任务条几何（周 → 月）", async () => {
    const { wrapper } = await mountView();

    const before = wrapper
      .find('[data-testid="tl-bar-T-002"]')
      .attributes("style");
    await wrapper.find('[data-testid="tl-scale-month"]').trigger("click");
    await flushPromises();
    const after = wrapper
      .find('[data-testid="tl-bar-T-002"]')
      .attributes("style");

    expect(before).toContain("54px");
    expect(after).toContain("18px");
  });

  it("拖拽移动任务条 → 下游顺延确认 → 排期生效", async () => {
    const { wrapper } = await mountView();
    const bar = wrapper.find('[data-testid="tl-bar-T-001"]');

    // 周比例尺 18px/天：+36px = 后移 2 天；下游 T-002/T-005/T-009 等随之顺延
    await bar.trigger("pointerdown", { clientX: 100, pointerId: 1 });
    await bar.trigger("pointermove", { clientX: 136, pointerId: 1 });
    await bar.trigger("pointerup", { pointerId: 1 });
    await flushPromises();

    const confirmButton = bodyButton("保存排期");
    expect(confirmButton).not.toBeNull();
    await confirmButton?.click();
    await flushPromises();

    expect(
      wrapper.find('[data-testid="tl-bar-T-001"]').attributes("data-start")
    ).toBe("2026-11-18");
    // 下游 T-002 同步顺延：11-19 → 11-21
    expect(
      wrapper.find('[data-testid="tl-bar-T-002"]').attributes("data-start")
    ).toBe("2026-11-21");
  });

  it("排期校验报告未排期任务", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="tl-validate"]').trigger("click");
    await flushPromises();

    const panel = wrapper.find('[data-testid="tl-validation"]');
    expect(panel.exists()).toBe(true);
    expect(panel.text()).toContain("3");
  });

  it("采纳 AI 排期建议走确认并移动任务条；保存版本出通知", async () => {
    const { wrapper } = await mountView();

    await wrapper
      .find('[data-testid="tl-rec-TL-1"] [type="button"]')
      .trigger("click");
    await flushPromises();
    await bodyButton("保存排期")?.click();
    await flushPromises();

    expect(
      wrapper.find('[data-testid="tl-bar-T-006"]').attributes("data-start")
    ).toBe("2026-11-25");

    await wrapper.find('[data-testid="tl-save"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-testid="tl-notice"]').exists()).toBe(true);
    // reka Select 不透传 attrs，比较入口以占位文案断言
    expect(wrapper.text()).toContain("比较版本");
  });
});
