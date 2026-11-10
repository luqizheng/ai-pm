// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import ExecutionReadinessView from "./ExecutionReadinessView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";

/**
 * 执行准备（S04-UI-07，mock 预览页）质量门测试：
 * 三类汇总 + 检查表联动、问题处置（解决 / 留痕豁免）、移交闸门
 * （未决问题禁用 → 全部闭账后可移交 → 确认出交付横幅）。
 */

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/readiness");
  await router.isReady();
  const wrapper = mountShell(
    ExecutionReadinessView,
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

function setBodyInput(testid: string, value: string) {
  const host = document.body.querySelector(`[data-testid="${testid}"]`);
  const input = (
    host?.matches("input") ? host : host?.querySelector("input")
  ) as HTMLInputElement | null;
  if (!input) throw new Error(`input not found: ${testid}`);
  input.value = value;
  input.dispatchEvent(new Event("input"));
}

beforeEach(() => {
  vi.useFakeTimers();
});

describe("ExecutionReadinessView", () => {
  it("初始存在未决问题：检查表打警戒态、移交被闸住", async () => {
    const { wrapper } = await mountView();

    expect(
      wrapper.find('[data-testid="rd-item-d1"] .rd-item-state').classes()
    ).toContain("rd-item-state--block");
    expect(
      wrapper.find('[data-testid="rd-item-e1"] .rd-item-state').classes()
    ).toContain("rd-item-state--warn");
    const handoff = wrapper.find('[data-testid="rd-handoff-btn"]');
    expect(handoff.attributes("disabled")).toBeDefined();
    expect(wrapper.find('[data-testid="rd-handoff-blocked"]').exists()).toBe(
      true
    );
  });

  it("校验动作更新上次校验时间", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="rd-validate"]').trigger("click");
    await vi.advanceTimersByTimeAsync(400);
    await flushPromises();

    expect(wrapper.find('[data-testid="rd-last"]').exists()).toBe(true);
  });

  it("问题可解决；全部闭账后移交解锁并出交付横幅", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="rd-resolve-I-1"]').trigger("click");
    await flushPromises();
    expect(
      wrapper.find('[data-testid="rd-issue-I-1"]').attributes("data-state")
    ).toBe("resolved");

    for (const id of ["I-2", "I-3", "I-4", "I-5"]) {
      await wrapper.find(`[data-testid="rd-resolve-${id}"]`).trigger("click");
      await flushPromises();
    }

    expect(wrapper.find('[data-testid="rd-handoff-blocked"]').exists()).toBe(
      false
    );
    expect(
      wrapper.find('[data-testid="rd-handoff-btn"]').attributes("disabled")
    ).toBeUndefined();

    await wrapper.find('[data-testid="rd-handoff-btn"]').trigger("click");
    await flushPromises();
    await bodyButton("确认移交")?.click();
    await flushPromises();

    expect(wrapper.find('[data-testid="rd-handoff-done"]').exists()).toBe(true);
  });

  it("豁免必须留痕：空理由被拒，填理由后问题闭账", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="rd-waive-I-5"]').trigger("click");
    await flushPromises();
    await bodyButton("确认豁免")?.click();
    await flushPromises();
    expect(
      document.body.querySelector('[data-testid="rd-waive-reason"]')
    ).not.toBeNull();
    // 空理由：确认后问题仍未闭账（错误提示）
    expect(
      wrapper.find('[data-testid="rd-issue-I-5"]').attributes("data-state")
    ).toBe("open");

    setBodyInput("rd-waive-reason", "上游裁决延后，本期按现状交付");
    await flushPromises();
    await bodyButton("确认豁免")?.click();
    await flushPromises();

    expect(
      wrapper.find('[data-testid="rd-issue-I-5"]').attributes("data-state")
    ).toBe("waived");
    expect(wrapper.text()).toContain("上游裁决延后");
  });
});
