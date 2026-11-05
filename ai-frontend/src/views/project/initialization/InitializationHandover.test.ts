// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";

import InitializationOverviewView from "./InitializationOverviewView.vue";
import { i18n } from "@/lib/i18n";
import type { InitializationOverview } from "@/api/initialization-overview";

/**
 * S01-07 交接出口并入 S01-01 的行为：
 *   - ready / partial 提供「进入项目总控台」主 CTA（→ /project/command-center）；
 *   - blocked / undetermined 不提供 CTA（不虚假开放交接）；
 *   - 会话内观测到 →ready 跃迁（如确认完基线后刷新）自动跳入总控台；
 *   - 首次加载即为 ready 时不跳——初始化始终可作为历史回访。
 */

const loadInitializationOverview = vi.fn();
vi.mock("@/api/initialization-overview", async importOriginal => {
  const actual = await importOriginal<
    typeof import("@/api/initialization-overview")
  >();
  return {
    ...actual,
    loadInitializationOverview: (...args: unknown[]) =>
      loadInitializationOverview(...args)
  };
});

function overviewOf(outcome: InitializationOverview["handoverOutcome"]) {
  return {
    projectId: "prj-shield",
    blocks: {},
    projectSummary: {
      name: "星河客服中台",
      code: "SHIELD",
      lifecycle: "planning",
      archived: false,
      customer_name: null,
      repository_count: 1,
      verified_repository_count: 0,
      member_count: 1,
      goal: { present: false }
    },
    stages: [],
    currentAction: {
      stageId: "handover",
      titleKey: "stage.handover",
      status: "completed",
      owner: "system",
      needsUser: false,
      remaining: 0
    },
    humanActions: [],
    aiContext: [],
    handoverOutcome: outcome,
    completion: "not_determinable"
  } as unknown as InitializationOverview;
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/project/initialization", component: InitializationOverviewView },
      { path: "/project/command-center", component: { template: "<div />" } }
    ]
  });
  await router.push("/project/initialization?project_id=prj-shield");
  await router.isReady();

  const wrapper = mount(InitializationOverviewView, {
    global: { plugins: [createPinia(), i18n, router] }
  });
  await flushPromises();
  return { wrapper, router };
}

describe("InitializationOverviewView · handover（S01-07 并入）", () => {
  it("undetermined：呈现接入概况，不提供进入总控台的 CTA", async () => {
    loadInitializationOverview.mockResolvedValue(overviewOf("undetermined"));
    const { wrapper } = await mountView();

    expect(
      wrapper.find('[data-testid="init-handover"]').attributes("data-outcome")
    ).toBe("undetermined");
    expect(
      wrapper.find('[data-testid="init-handover-enter"]').exists()
    ).toBe(false);
    expect(wrapper.text()).not.toContain("初始化完成");
  });

  it("blocked：说明阻塞，不提供 CTA", async () => {
    loadInitializationOverview.mockResolvedValue(overviewOf("blocked"));
    const { wrapper } = await mountView();

    expect(
      wrapper.find('[data-testid="init-handover"]').attributes("data-outcome")
    ).toBe("blocked");
    expect(
      wrapper.find('[data-testid="init-handover-enter"]').exists()
    ).toBe(false);
    expect(wrapper.text()).toContain("存在阻塞事项");
  });

  it("ready：提供进入总控台 CTA；首次加载即为 ready 不自动跳（保护历史回访）", async () => {
    loadInitializationOverview.mockResolvedValue(overviewOf("ready"));
    const { wrapper, router } = await mountView();
    const push = vi.spyOn(router, "push").mockResolvedValue(undefined);

    expect(
      wrapper.find('[data-testid="init-handover"]').attributes("data-outcome")
    ).toBe("ready");
    expect(wrapper.text()).toContain("交接完成后，初始化仍作为项目历史保留在此");
    expect(push).not.toHaveBeenCalled();

    await wrapper.find('[data-testid="init-handover-enter"]').trigger("click");
    expect(push).toHaveBeenCalledWith("/project/command-center");
  });

  it("会话内观测到 →ready 跃迁（如确认基线后刷新）自动跳入总控台", async () => {
    vi.useFakeTimers();
    try {
      loadInitializationOverview
        .mockResolvedValueOnce(overviewOf("undetermined"))
        .mockResolvedValueOnce(overviewOf("ready"));
      const { wrapper, router } = await mountView();
      const push = vi.spyOn(router, "push").mockResolvedValue(undefined);

      // 首载 undetermined → 手动刷新得到 ready
      loadInitializationOverview.mockResolvedValue(overviewOf("ready"));
      await wrapper.findAll("button")[0].trigger("click"); // Reload（actions 槽第一枚按钮）
      await flushPromises();
      vi.advanceTimersByTime(1300);
      await flushPromises();

      expect(push).toHaveBeenCalledWith("/project/command-center");
    } finally {
      vi.useRealTimers();
    }
  });
});
